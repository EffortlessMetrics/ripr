//! One closed authority for source-visible Rust `cfg` / `cfg_attr`
//! test-gate classification (#3530, RIPR-SPEC-0153 family).
//!
//! Before this module the parser producer (`analysis::syntax::ra`) and the
//! facts normalizer (`super::test_styles`) each carried a partial copy of the
//! cfg-term rules, and the copies drifted: whitespace and multi-line
//! attribute spellings were demoted by the normalizer's line walk, and nested
//! `all(test, ...)` conjunctions were unrecognized everywhere. This module is
//! now the sole predicate authority both sides consume.
//!
//! The classification is a closed result (see [`CfgTestRequirement`]) over
//! structural token shapes only:
//!
//! - `cfg(test)` and `cfg(all(..., test, ...))` — at any nesting depth, any
//!   conjunct order — structurally require a test build;
//! - `any(...)` alternatives admit a test build among non-test builds
//!   ([`CfgTestRequirement::MayIncludeTest`]), `not(...)` and key-value
//!   predicates are provably independent of test;
//! - `cfg_attr` conditionally introduces attributes: it never gates the item
//!   itself on test, so it classifies at most
//!   [`CfgTestRequirement::MayIncludeTest`];
//! - comments, string/raw-string/byte-string literals, and lookalike
//!   identifiers (`test_support`, feature values containing `test`) cannot
//!   influence the result;
//! - unsupported, malformed, or unbalanced input is
//!   [`CfgTestRequirement::Unknown`] and never guesses from substrings.
//!
//! The result intentionally says nothing about which Cargo features or
//! targets are active; feature activation is not established statically.

/// Closed classification of one source-visible configuration attribute's
/// effect on an item's test-build requirement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CfgTestRequirement {
    /// A structurally test-required gate: the item only exists in test
    /// builds.
    RequiresTest,
    /// A test build may include the item, but a non-test build can too.
    MayIncludeTest,
    /// The attribute provably does not make the item test-only.
    IndependentOfTest,
    /// Unsupported, malformed, or structurally ambiguous input.
    Unknown,
}

use CfgTestRequirement::{
    IndependentOfTest as Independent, MayIncludeTest as MayInclude, RequiresTest, Unknown,
};

#[derive(Debug, PartialEq, Eq)]
enum Token {
    /// An identifier or keyword (`cfg`, `all`, `test`, `target_os`, ...).
    Word(String),
    /// One ASCII punctuation character from the bounded allowed set.
    Punct(char),
    /// An opaque string, raw string, byte string, or numeric literal. Its
    /// text is deliberately unavailable so literal content can never
    /// manufacture a `test` term.
    Literal,
}

struct PositionedToken {
    offset: usize,
    token: Token,
}

/// Classifies one full attribute's text (`#[cfg(test)]`,
/// `#[cfg_attr(test, allow(dead_code))]`, `#![cfg(test)]`, `#[allow(...)]`).
///
/// Any well-formed attribute that is not `cfg`/`cfg_attr` is
/// [`CfgTestRequirement::IndependentOfTest`]: it does not gate item
/// availability. Malformed input is [`CfgTestRequirement::Unknown`].
pub(crate) fn classify_attribute(attr_text: &str) -> CfgTestRequirement {
    let Some(tokens) = positioned_tokens(attr_text) else {
        return Unknown;
    };
    let plain: Vec<Token> = tokens
        .into_iter()
        .map(|positioned| positioned.token)
        .collect();
    let body = match plain.as_slice() {
        [
            Token::Punct('#'),
            Token::Punct('['),
            body @ ..,
            Token::Punct(']'),
        ] => body,
        [
            Token::Punct('#'),
            Token::Punct('!'),
            Token::Punct('['),
            body @ ..,
            Token::Punct(']'),
        ] => body,
        // The caller's contract is attribute text, so an unrecognizable
        // envelope fails closed instead of guessing.
        _ => return Unknown,
    };
    if !body_is_balanced(body) {
        return Unknown;
    }
    match body {
        [
            Token::Word(path),
            Token::Punct('('),
            inner @ ..,
            Token::Punct(')'),
        ] if body_is_balanced(inner) => {
            match path.as_str() {
                "cfg" => classify_predicate(inner),
                "cfg_attr" => classify_cfg_attr_arguments(inner),
                // Any other well-formed attribute cannot gate availability
                // on test.
                _ => Independent,
            }
        }
        // cfg-shaped but unparseable: fail closed.
        [Token::Word(path), ..] if path == "cfg" || path == "cfg_attr" => Unknown,
        _ => Independent,
    }
}

/// True when the attribute's classification is
/// [`CfgTestRequirement::RequiresTest`] — the shared answer the parser
/// producer and the facts normalizer both consume for "does this attribute
/// gate the item on test?".
pub(crate) fn attribute_requires_test(attr_text: &str) -> bool {
    classify_attribute(attr_text) == RequiresTest
}

/// Conjunctively composes every attribute on one item. Rust applies multiple
/// `cfg` gates conjunctively, so a definitely test-required gate stays
/// test-required next to independent gates, and an unknown gate cannot
/// strengthen an otherwise non-test item by optimism.
pub(crate) fn attributes_require_test<I>(attributes: I) -> bool
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    attributes
        .into_iter()
        .fold(Independent, |combined, attribute| {
            conjunctive(combined, classify_attribute(attribute.as_ref()))
        })
        == RequiresTest
}

/// Splits one line (or joined continuation lines) at the end of its leading
/// attribute, skipping strings, raw strings, and comments while matching
/// brackets. Returns the attribute text (including `#`/`#![` and `]`) and
/// the unconsumed remainder. `None` means no complete leading attribute is
/// present — callers treat that as no gate credit (fail closed).
pub(crate) fn split_leading_attribute(text: &str) -> Option<(&str, &str)> {
    let leading = text.trim_start();
    if !leading.starts_with("#[") && !leading.starts_with("#![") {
        return None;
    }
    let attribute_start = text.len() - leading.len();
    // The line often continues after the attribute (`#[cfg(test)] mod
    // módulos {`), and the remainder may hold non-ASCII Rust syntax the
    // fail-closed lexer cannot traverse. Bound the lexing to the attribute
    // itself: find its closing bracket with a string-aware byte scan, then
    // lex only those bytes.
    let end = attribute_start + leading_attribute_byte_len(leading)?;
    let attribute = text.get(attribute_start..end)?;
    let tokens = positioned_tokens(attribute)?;
    let mut depth = 0usize;
    for positioned in &tokens {
        match positioned.token {
            Token::Punct('[') => depth = depth.saturating_add(1),
            Token::Punct(']') => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    // Token offsets are attribute-relative: the attribute
                    // slice is bounded by them, while the remainder stays
                    // anchored to the full line.
                    let end = positioned.offset + ']'.len_utf8();
                    return Some((attribute.get(..end)?, text.get(attribute_start + end..)?));
                }
            }
            _ => {}
        }
    }
    None
}

/// Byte length of the leading `#[...]` attribute, tracking bracket depth
/// with string-literal awareness (raw-string hashes are inert to depth).
/// Returns `None` when the brackets never balance.
fn leading_attribute_byte_len(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = 0usize;
    let mut depth = 0usize;
    let mut in_string: Option<u8> = None;
    while index < bytes.len() {
        let byte = bytes[index];
        if let Some(quote) = in_string {
            match byte {
                b'\\' => index += 2,
                _ if byte == quote => {
                    in_string = None;
                    index += 1;
                }
                _ => index += 1,
            }
            continue;
        }
        match byte {
            b'"' | b'\'' => {
                in_string = Some(byte);
                index += 1;
            }
            b'[' | b'(' => {
                depth += 1;
                index += 1;
            }
            b']' | b')' => {
                depth = depth.checked_sub(1)?;
                index += 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => index += 1,
        }
    }
    None
}

/// Nesting depth bound for predicate recursion. Real cfg expressions nest a
/// handful of levels; anything past this bound is exotic input that fails
/// closed rather than risking unbounded recursion on the stack.
const MAX_PREDICATE_NESTING: usize = 64;

/// Classifies the token sequence inside `cfg(...)`.
fn classify_predicate(tokens: &[Token]) -> CfgTestRequirement {
    classify_predicate_at_depth(tokens, 0)
}

fn classify_predicate_at_depth(tokens: &[Token], depth: usize) -> CfgTestRequirement {
    if depth > MAX_PREDICATE_NESTING {
        return Unknown;
    }
    let depth = depth + 1;
    match tokens {
        [Token::Word(option)] if option == "test" => RequiresTest,
        // A bare custom option (`cfg(unix)`) is provably not a test gate.
        [Token::Word(_)] => Independent,
        // Key-value predicates (`cfg(target_os = "test")`) never require
        // test: the literal is opaque and cannot manufacture a term.
        [Token::Word(_), Token::Punct('='), Token::Literal] => Independent,
        [
            Token::Word(gate),
            Token::Punct('('),
            inner @ ..,
            Token::Punct(')'),
        ] if body_is_balanced(inner) => {
            let branches: Vec<CfgTestRequirement> = split_top_level_commas(inner)
                .into_iter()
                .filter(|branch| !branch.is_empty())
                .map(|branch| classify_predicate_at_depth(branch, depth))
                .collect();
            if branches.is_empty() {
                // `cfg(all())` / `cfg(any())` are not well-formed Rust.
                return Unknown;
            }
            match gate.as_str() {
                "all" => branches.into_iter().fold(Independent, conjunctive),
                "any" => branches.into_iter().fold(Independent, disjunctive),
                "not" => {
                    // Negation can never structurally require test; an
                    // unreadable operand stays unknown. The operand shares
                    // this frame's depth budget so chained `not`s cannot
                    // restart the recursion counter.
                    match classify_predicate_at_depth(inner, depth) {
                        Unknown => Unknown,
                        _ => Independent,
                    }
                }
                _ => Unknown,
            }
        }
        _ => Unknown,
    }
}

/// Classifies the token sequence inside `cfg_attr(...)`.
///
/// `cfg_attr(condition, introduced...)` never gates the item itself on test:
/// when the condition fails the item stays present without the introduced
/// attributes. A conditional introduction of a test-requiring `cfg` gate
/// therefore classifies at most [`CfgTestRequirement::MayIncludeTest`] —
/// feature activation is not established statically.
fn classify_cfg_attr_arguments(arguments: &[Token]) -> CfgTestRequirement {
    let parts = split_top_level_commas(arguments);
    let Some((condition, introduced)) = parts.split_first() else {
        return Unknown;
    };
    let introduced: Vec<&[Token]> = introduced
        .iter()
        .copied()
        .filter(|part| !part.is_empty())
        .collect();
    if introduced.is_empty() {
        // `cfg_attr(P)` with no introduced attribute is malformed.
        return Unknown;
    }
    match classify_predicate(condition) {
        Unknown => Unknown,
        _ => {
            let introduces_test_gate = introduced.iter().any(|attribute| {
                matches!(
                    classify_introduced_cfg_gate(attribute),
                    Some(MayInclude | RequiresTest)
                )
            });
            if introduces_test_gate {
                MayInclude
            } else {
                Independent
            }
        }
    }
}

/// True when any attribute in the set is a `cfg_attr` that conditionally
/// introduces a `path` attribute on the item (#3533).
///
/// `#[cfg_attr(condition, path = "...")]` makes the item's module file target
/// configuration-dependent: the compiler loads the introduced target when the
/// condition holds and the default layout (or nothing) otherwise, so no
/// single static target exists. The module-path producer treats such
/// declarations as a typed unknown instead of resolving the default file,
/// which Rust would not compile under the conditional configuration.
///
/// Structural detection only: `path` must introduce an attribute at a
/// `cfg_attr` argument position. String contents are opaque tokens, so
/// `cfg(feature = "path")` conditions and comments cannot trigger it. Nested
/// `cfg_attr` introductions are followed (their conditions skipped, depth
/// bounded) because a missed conditional `path` would resolve the wrong
/// module file — the over-emission direction. Unreadable nesting beyond the
/// bound fails closed to `true`: unreadable input could hide a path
/// introduction.
pub(crate) fn attributes_conditionally_introduce_path<I>(attributes: I) -> bool
where
    I: IntoIterator,
    I::Item: AsRef<str>,
{
    attributes
        .into_iter()
        .any(|attribute| attribute_conditionally_introduces_path(attribute.as_ref()))
}

fn attribute_conditionally_introduces_path(attribute: &str) -> bool {
    let Some(tokens) = positioned_tokens(attribute) else {
        // The bounded ASCII lexer cannot read this attribute. Rust accepts
        // Unicode identifiers in predicate position (XID_Start/XID_Continue,
        // e.g. `#[cfg_attr(é, path = "alternate.rs")]`), so an unreadable
        // `cfg_attr` could hide a conditional `path` introduction that would
        // resolve the wrong module file. Fail closed only for attributes
        // whose head is recognizably `cfg_attr`; any other unlexable
        // attribute cannot introduce a module path.
        return unlexable_attribute_head_is_cfg_attr(attribute);
    };
    let plain: Vec<Token> = tokens
        .into_iter()
        .map(|positioned| positioned.token)
        .collect();
    let body = match plain.as_slice() {
        [
            Token::Punct('#'),
            Token::Punct('['),
            body @ ..,
            Token::Punct(']'),
        ] => body,
        [
            Token::Punct('#'),
            Token::Punct('!'),
            Token::Punct('['),
            body @ ..,
            Token::Punct(']'),
        ] => body,
        _ => return false,
    };
    if !body_is_balanced(body) {
        return false;
    }
    match body {
        [
            Token::Word(name),
            Token::Punct('('),
            inner @ ..,
            Token::Punct(')'),
        ] if name == "cfg_attr" && body_is_balanced(inner) => {
            cfg_attr_arguments_introduce_path(inner, 0)
        }
        _ => false,
    }
}

/// Walks `cfg_attr(condition, introduced...)` argument tokens at a bounded
/// depth: an introduced attribute whose head is the word `path` before `=`
/// marks a conditional module path. Nested `cfg_attr` introductions are
/// followed (their conditions skipped) for the same fail-closed reason.
fn cfg_attr_arguments_introduce_path(arguments: &[Token], depth: usize) -> bool {
    if depth > MAX_PREDICATE_NESTING {
        // Unreadable nesting could hide a `path` introduction: fail closed.
        return true;
    }
    for part in split_top_level_commas(arguments).iter().skip(1) {
        match part {
            // `path = "..."` and the bare attribute name both introduce a
            // `path` attribute; either makes the target conditional.
            [Token::Word(name), Token::Punct('='), ..] if name == "path" => return true,
            [Token::Word(name)] if name == "path" => return true,
            [
                Token::Word(name),
                Token::Punct('('),
                inner @ ..,
                Token::Punct(')'),
            ] if name == "cfg_attr"
                && body_is_balanced(inner)
                && cfg_attr_arguments_introduce_path(inner, depth + 1) =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Recognizes a directly introduced `cfg(...)` attribute inside `cfg_attr`
/// and classifies its predicate. Nested `cfg_attr` introductions are a
/// documented bound: they are not credited as test gates here.
fn classify_introduced_cfg_gate(tokens: &[Token]) -> Option<CfgTestRequirement> {
    match tokens {
        [
            Token::Word(path),
            Token::Punct('('),
            inner @ ..,
            Token::Punct(')'),
        ] if path == "cfg" && body_is_balanced(inner) => Some(classify_predicate(inner)),
        _ => None,
    }
}

/// Bounded envelope check for attributes the ASCII lexer cannot read:
/// true only when the attribute head is recognizably `cfg_attr`
/// (`#[cfg_attr(` or `#![cfg_attr(`, whitespace-insensitive). This is a
/// byte-prefix shape test, not a parse: it never needs to traverse the
/// non-ASCII argument bytes that defeated the lexer, and `cfg_attr`-prefixed
/// lookalike heads (`cfg_attrs`, `my_cfg_attr`) do not match.
fn unlexable_attribute_head_is_cfg_attr(attribute: &str) -> bool {
    let body = attribute.trim_start();
    let body = body.strip_prefix('#').unwrap_or(body);
    let body = body.strip_prefix('!').unwrap_or(body);
    let Some(body) = body.strip_prefix('[') else {
        return false;
    };
    let Some(rest) = body.trim_start().strip_prefix("cfg_attr") else {
        return false;
    };
    rest.trim_start().starts_with('(')
}

/// Conjunction: a definitely test-required branch dominates; an unknown
/// branch blocks optimism; a possible-test branch keeps the possibility.
fn conjunctive(left: CfgTestRequirement, right: CfgTestRequirement) -> CfgTestRequirement {
    if left == RequiresTest || right == RequiresTest {
        RequiresTest
    } else if left == Unknown || right == Unknown {
        Unknown
    } else if left == MayInclude || right == MayInclude {
        MayInclude
    } else {
        Independent
    }
}

/// Disjunction: only an all-requiring alternative requires test; an
/// all-independent alternative is provably independent; anything else may
/// include a test build.
fn disjunctive(left: CfgTestRequirement, right: CfgTestRequirement) -> CfgTestRequirement {
    if left == RequiresTest && right == RequiresTest {
        RequiresTest
    } else if left == Independent && right == Independent {
        Independent
    } else {
        MayInclude
    }
}

/// Splits a predicate's tokens at top-level commas. Literal tokens are
/// opaque, so commas inside strings never split terms.
fn split_top_level_commas(tokens: &[Token]) -> Vec<&[Token]> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        match token {
            Token::Punct('(') | Token::Punct('[') => depth = depth.saturating_add(1),
            Token::Punct(')') | Token::Punct(']') => depth = depth.saturating_sub(1),
            Token::Punct(',') if depth == 0 => {
                parts.push(&tokens[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&tokens[start..]);
    parts
}

/// True when brackets and parentheses never go negative and close fully.
fn body_is_balanced(body: &[Token]) -> bool {
    let mut brackets = 0isize;
    let mut parens = 0isize;
    for token in body {
        match token {
            Token::Punct('[') => brackets += 1,
            Token::Punct(']') => {
                brackets -= 1;
                if brackets < 0 {
                    return false;
                }
            }
            Token::Punct('(') => parens += 1,
            Token::Punct(')') => {
                parens -= 1;
                if parens < 0 {
                    return false;
                }
            }
            _ => {}
        }
    }
    brackets == 0 && parens == 0
}

/// Lexes attribute text into bounded tokens, skipping whitespace and
/// comments and treating string-like literals as opaque. Character/lifetime
/// quotes and non-ASCII or control bytes fail closed (`None`) so exotic
/// spellings classify as `Unknown` instead of guessing.
fn positioned_tokens(text: &str) -> Option<Vec<PositionedToken>> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        let start = index;
        let Some(&byte) = bytes.get(index) else {
            break;
        };
        let pushed = match byte {
            b' ' | b'\t' | b'\r' | b'\n' => {
                index += 1;
                None
            }
            b'/' if bytes.get(index + 1) == Some(&b'/') => {
                index += 2;
                while bytes.get(index).is_some_and(|current| *current != b'\n') {
                    index += 1;
                }
                None
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index = skip_block_comment(bytes, index)?;
                None
            }
            b'"' => {
                index = skip_regular_string(bytes, index)?;
                Some(Token::Literal)
            }
            b'\'' => {
                // Character vs lifetime quotes are ambiguous without a
                // parser; the cfg shapes this authority supports never
                // contain them, so fail closed.
                return None;
            }
            _ if byte == b'b' && bytes.get(index + 1) == Some(&b'"') => {
                index = skip_regular_string(bytes, index + 1)?;
                Some(Token::Literal)
            }
            _ if raw_string_start(bytes, index).is_some() => {
                let (quote_index, hashes) = raw_string_start(bytes, index).unwrap_or((index, 0));
                index = skip_raw_string(bytes, quote_index, hashes)?;
                Some(Token::Literal)
            }
            _ if byte.is_ascii_alphabetic() || byte == b'_' => {
                let mut end = index;
                while bytes
                    .get(end)
                    .is_some_and(|current| current.is_ascii_alphanumeric() || *current == b'_')
                {
                    end += 1;
                }
                let word = text.get(index..end)?;
                index = end;
                Some(Token::Word(word.to_string()))
            }
            _ if byte.is_ascii_digit() => {
                let mut end = index;
                while bytes.get(end).is_some_and(|current| {
                    current.is_ascii_alphanumeric() || *current == b'_' || *current == b'.'
                }) {
                    end += 1;
                }
                index = end;
                Some(Token::Literal)
            }
            // Remaining ASCII graphics (code punctuation around the attribute
            // on a source line, e.g. `{`) become inert punctuation; the
            // structural classifiers only match exact cfg shapes.
            _ if byte.is_ascii_graphic() => {
                index += 1;
                Some(Token::Punct(byte as char))
            }
            // Any other byte (multi-byte UTF-8, control bytes) fails closed.
            _ => return None,
        };
        if let Some(token) = pushed {
            tokens.push(PositionedToken {
                offset: start,
                token,
            });
        }
    }
    Some(tokens)
}

/// Returns the opening-quote offset and hash count when the bytes at `index`
/// start a raw string (`r"..."`, `r#"..."#`, `br##"..."##`), otherwise
/// `None`.
fn raw_string_start(bytes: &[u8], index: usize) -> Option<(usize, usize)> {
    let raw_index = match bytes.get(index) {
        Some(b'r') => index,
        Some(b'b') if bytes.get(index + 1) == Some(&b'r') => index + 1,
        _ => return None,
    };
    let mut hashes = 0usize;
    let mut cursor = raw_index + 1;
    while bytes.get(cursor) == Some(&b'#') {
        hashes += 1;
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'"') {
        Some((cursor, hashes))
    } else {
        None
    }
}

/// Returns the offset just past a `"..."` string whose opening quote is at
/// `index`. Escaped quotes do not close the string; EOF fails closed.
fn skip_regular_string(bytes: &[u8], index: usize) -> Option<usize> {
    let mut cursor = index.checked_add(1)?;
    while bytes.get(cursor).is_some() {
        match bytes.get(cursor) {
            Some(b'\\') => cursor = cursor.checked_add(2)?,
            Some(b'"') => return cursor.checked_add(1),
            Some(_) => cursor += 1,
            None => return None,
        }
    }
    None
}

/// Returns the offset just past a raw string whose opening quote is at
/// `index`, closed by `"` followed by exactly `hashes` `#` characters.
fn skip_raw_string(bytes: &[u8], index: usize, hashes: usize) -> Option<usize> {
    let mut cursor = index.checked_add(1)?;
    while bytes.get(cursor).is_some() {
        if bytes.get(cursor) == Some(&b'"') {
            let mut end = cursor.checked_add(1)?;
            let mut remaining = hashes;
            while remaining > 0 && bytes.get(end) == Some(&b'#') {
                end += 1;
                remaining -= 1;
            }
            if remaining == 0 {
                return Some(end);
            }
        }
        cursor += 1;
    }
    None
}

/// Returns the offset just past a (possibly nested) block comment starting
/// with `/*` at `index`. Unterminated comments fail closed.
fn skip_block_comment(bytes: &[u8], index: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut cursor = index.checked_add(2)?;
    while bytes.get(cursor).is_some() {
        if bytes.get(cursor) == Some(&b'/') && bytes.get(cursor + 1) == Some(&b'*') {
            depth += 1;
            cursor += 2;
        } else if bytes.get(cursor) == Some(&b'*') && bytes.get(cursor + 1) == Some(&b'/') {
            depth = depth.checked_sub(1)?;
            cursor += 2;
            if depth == 0 {
                return Some(cursor);
            }
        } else {
            cursor += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests;
