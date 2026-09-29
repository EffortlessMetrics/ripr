//! Nesting budget for Rust parses.
//!
//! `ra_ap_parser` is recursive descent, and both ripr's syntax walkers and
//! rowan's tree drop recurse once per tree level. Rust files are parsed on
//! rayon workers (2 MiB stacks) and parse trees are dropped on the
//! `ParseNodeDropper` thread `ra_ap_syntax` spawns with a default stack, so
//! a deeply nested file anywhere in the workspace aborted the whole process
//! with a stack overflow, even when the file was not in the diff. Measured
//! abort points on Linux x86_64: about 2,000 nested blocks or `if`s and
//! 4,000 nested parens, closures or prefix operators in release builds;
//! about 600 nested blocks in debug builds; 16,000-branch `else if` chains
//! and 64,000-term operator chains in release builds.
//!
//! Every `SourceFile::parse` in the analyzer goes through
//! [`parse_clean_source_file`] or checks [`rust_nesting_refusal`] first. A
//! refused source is handled exactly like a source with parse errors: each
//! caller already fails closed on that path (lexical fallback for file
//! facts, typed limitations or no evidence elsewhere).

use ra_ap_syntax::{Edition, Parse, SourceFile};

/// Maximum estimated bracket depth plus prefix-operator run. Real Rust
/// rarely nests past a few dozen levels; the budget sits well below the
/// measured debug-build abort depth (about 600 nested blocks on a 2 MiB
/// worker stack).
pub(crate) const RUST_NESTING_BUDGET: usize = 256;

/// Maximum `else if` links in one chain. Each link nests the next `if`
/// inside the previous one's `else` branch.
pub(crate) const RUST_ELSE_IF_CHAIN_BUDGET: usize = 2_048;

/// Maximum binary-operator characters in one run between `;`, `,`, `{` and
/// `}` at the same bracket depth. A left-associative chain nests one tree
/// level per operator.
pub(crate) const RUST_OPERATOR_RUN_BUDGET: usize = 4_096;

/// Parse `text` as a Rust source file when it is within the nesting budget
/// and parses without errors; `None` otherwise.
pub(crate) fn parse_clean_source_file(text: &str) -> Option<Parse<SourceFile>> {
    if rust_nesting_refusal(text).is_some() {
        return None;
    }
    let parse = SourceFile::parse(text, Edition::CURRENT);
    parse.errors().is_empty().then_some(parse)
}

/// Typed `rust_nesting_budget` static-limit reason when `text` is over any
/// nesting budget, else `None`.
pub(crate) fn rust_nesting_refusal(text: &str) -> Option<String> {
    let estimate = estimate_rust_nesting(text);
    if estimate.depth > RUST_NESTING_BUDGET {
        return Some(format!(
            "static limit rust_nesting_budget: estimated nesting depth {} exceeds the {RUST_NESTING_BUDGET}-level parse budget; parse refused to avoid a stack overflow",
            estimate.depth
        ));
    }
    if estimate.else_if_chain > RUST_ELSE_IF_CHAIN_BUDGET {
        return Some(format!(
            "static limit rust_nesting_budget: `else if` chain of {} links exceeds the {RUST_ELSE_IF_CHAIN_BUDGET}-link parse budget; parse refused to avoid a stack overflow",
            estimate.else_if_chain
        ));
    }
    if estimate.operator_run > RUST_OPERATOR_RUN_BUDGET {
        return Some(format!(
            "static limit rust_nesting_budget: operator chain of {} operators exceeds the {RUST_OPERATOR_RUN_BUDGET}-operator parse budget; parse refused to avoid a stack overflow",
            estimate.operator_run
        ));
    }
    None
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct NestingEstimate {
    pub(crate) depth: usize,
    pub(crate) else_if_chain: usize,
    pub(crate) operator_run: usize,
}

/// One left-to-right byte scan; a lexical approximation, not a parse.
/// Comments, string, raw string and char literals are skipped so their
/// contents cannot inflate the estimate. Over-estimates only cost a lexical
/// fallback; the scan never recurses, so it is safe on any input.
pub(crate) fn estimate_rust_nesting(text: &str) -> NestingEstimate {
    let bytes = text.as_bytes();
    let mut estimate = NestingEstimate::default();
    let mut depth = 0usize;
    let mut unary_run = 0usize;
    // Per-depth counters, indexed by bracket depth.
    let mut else_if: Vec<usize> = vec![0];
    let mut operators: Vec<usize> = vec![0];
    // Sum of `operators` over the open-bracket stack: the length of the
    // operator chain the current token extends. A run before `(` continues
    // into it, and a `(..)` or `[..]` run continues after it closes, so
    // `((x + 1 ..) + 1 ..)` nested deep counts as one chain.
    let mut open_operators = 0usize;
    // True when the previous significant token ends an operand, so a
    // following `-`, `*`, `&` or `!` is binary, not prefix.
    let mut after_operand = false;
    let mut previous_word_was_else = false;
    let mut index = 0usize;

    while let Some(&byte) = bytes.get(index) {
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        let next = bytes.get(index + 1).copied();
        if byte == b'/' && next == Some(b'/') {
            index = skip_line(bytes, index);
            continue;
        }
        if byte == b'/' && next == Some(b'*') {
            index = skip_block_comment(bytes, index);
            continue;
        }
        if byte == b'"' {
            index = skip_string(bytes, index + 1);
            after_operand = true;
            unary_run = 0;
            previous_word_was_else = false;
            continue;
        }
        if byte == b'\'' {
            index = skip_char_or_lifetime(text, index);
            after_operand = true;
            unary_run = 0;
            previous_word_was_else = false;
            continue;
        }
        if byte.is_ascii_alphanumeric() || byte == b'_' || byte >= 0x80 {
            let start = index;
            while bytes
                .get(index)
                .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b >= 0x80)
            {
                index += 1;
            }
            let word = text.get(start..index).unwrap_or("");
            if matches!(word, "r" | "br" | "cr")
                && let Some(end) = skip_raw_string(bytes, index)
            {
                index = end;
                after_operand = true;
                unary_run = 0;
                previous_word_was_else = false;
                continue;
            }
            if word == "if"
                && let Some(chain) = else_if.get_mut(depth)
            {
                // A bare `if` starts a new chain at this depth.
                *chain = if previous_word_was_else {
                    chain.saturating_add(1)
                } else {
                    0
                };
                estimate.else_if_chain = estimate.else_if_chain.max(*chain);
            }
            previous_word_was_else = word == "else";
            after_operand = !is_expression_keyword(word);
            if after_operand {
                unary_run = 0;
            }
            continue;
        }
        previous_word_was_else = false;
        match byte {
            b'(' | b'[' | b'{' => {
                depth = depth.saturating_add(1);
                else_if.push(0);
                operators.push(0);
                after_operand = false;
                estimate.depth = estimate.depth.max(depth.saturating_add(unary_run));
            }
            b')' | b']' | b'}' => {
                depth = depth.saturating_sub(1);
                else_if.truncate(depth + 1);
                let closed: usize = operators.drain(depth + 1..).sum();
                if let Some(run) = operators.get_mut(depth) {
                    if byte == b'}' {
                        // A block ends the enclosing chain.
                        open_operators = open_operators.saturating_sub(closed + *run);
                        *run = 0;
                    } else {
                        *run = run.saturating_add(closed);
                    }
                }
                after_operand = true;
                unary_run = 0;
            }
            b';' | b',' => {
                if let Some(chain) = else_if.get_mut(depth) {
                    *chain = 0;
                }
                if let Some(run) = operators.get_mut(depth) {
                    open_operators = open_operators.saturating_sub(*run);
                    *run = 0;
                }
                after_operand = false;
                unary_run = 0;
            }
            b'!' | b'-' | b'*' | b'&' if !after_operand => {
                unary_run = unary_run.saturating_add(1);
                estimate.depth = estimate.depth.max(depth.saturating_add(unary_run));
            }
            // Postfix `.` and `?` count too: a long method or field chain is
            // a left-nested tree just like a binary-operator run.
            b'.' | b'?' | b'+' | b'-' | b'*' | b'/' | b'%' | b'^' | b'|' | b'&' | b'<' | b'>'
            | b'=' => {
                if let Some(run) = operators.get_mut(depth) {
                    *run = run.saturating_add(1);
                    open_operators = open_operators.saturating_add(1);
                    estimate.operator_run = estimate.operator_run.max(open_operators);
                }
                after_operand = byte == b'?';
            }
            _ => {
                after_operand = false;
            }
        }
        index += 1;
    }
    estimate
}

/// Words after which a `-`, `*`, `&` or `!` is a prefix operator.
fn is_expression_keyword(word: &str) -> bool {
    matches!(
        word,
        "return" | "break" | "in" | "if" | "else" | "match" | "while" | "let" | "mut" | "move"
    )
}

fn skip_line(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index).is_some_and(|b| *b != b'\n') {
        index += 1;
    }
    index
}

/// Rust block comments nest.
fn skip_block_comment(bytes: &[u8], mut index: usize) -> usize {
    let mut open = 0usize;
    while let Some(&byte) = bytes.get(index) {
        let next = bytes.get(index + 1).copied();
        if byte == b'/' && next == Some(b'*') {
            open += 1;
            index += 2;
        } else if byte == b'*' && next == Some(b'/') {
            open = open.saturating_sub(1);
            index += 2;
            if open == 0 {
                return index;
            }
        } else {
            index += 1;
        }
    }
    index
}

/// `index` is just past the opening quote; returns the index past the
/// closing quote.
fn skip_string(bytes: &[u8], mut index: usize) -> usize {
    while let Some(&byte) = bytes.get(index) {
        match byte {
            b'\\' => index += 2,
            b'"' => return index + 1,
            _ => index += 1,
        }
    }
    index
}

/// `index` is just past an `r`, `br` or `cr` prefix. Returns the index past
/// the raw string when one starts here.
fn skip_raw_string(bytes: &[u8], mut index: usize) -> Option<usize> {
    let mut hashes = 0usize;
    while bytes.get(index) == Some(&b'#') {
        hashes += 1;
        index += 1;
    }
    if bytes.get(index) != Some(&b'"') {
        return None;
    }
    index += 1;
    while let Some(&byte) = bytes.get(index) {
        index += 1;
        if byte == b'"' {
            let closing = bytes.get(index..index + hashes);
            if closing.is_some_and(|run| run.iter().all(|b| *b == b'#')) {
                return Some(index + hashes);
            }
        }
    }
    Some(index)
}

/// A char literal (`'x'`, `'\n'`, `'é'`) or a lifetime/label (`'a`).
fn skip_char_or_lifetime(text: &str, index: usize) -> usize {
    let bytes = text.as_bytes();
    if bytes.get(index + 1) == Some(&b'\\') {
        // Skip the escaped character itself so `'\''` closes correctly.
        let mut cursor = index + 3;
        while bytes
            .get(cursor)
            .is_some_and(|b| *b != b'\'' && *b != b'\n')
        {
            cursor += 1;
        }
        return cursor + 1;
    }
    let width = text
        .get(index + 1..)
        .and_then(|rest| rest.chars().next())
        .map_or(0, char::len_utf8);
    if width > 0 && bytes.get(index + 1 + width) == Some(&b'\'') {
        return index + 2 + width;
    }
    index + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nested(open: &str, close: &str, levels: usize) -> String {
        format!(
            "pub fn f(x: i32) -> i32 {{ {}x{} }}\n",
            open.repeat(levels),
            close.repeat(levels)
        )
    }

    #[test]
    fn ordinary_rust_is_within_budget() {
        let source = r####"
            // comment with ((((((( and "quotes"
            /* nested /* block */ ((((( */
            pub fn f<'a>(x: &'a [u8], y: &mut Vec<Option<u8>>) -> Result<(), String> {
                let s = "(((((((((((\"(((";
                let r = r#"((((("(((("#;
                let c = '(';
                let q = '\'';
                if x.len() > 3 && !y.is_empty() { return Err(format!("{}", -1)); }
                else if x.is_empty() { *y.first_mut().unwrap_or(&mut None) = None; }
                'outer: loop { break 'outer; }
                Ok(())
            }
        "####;
        let estimate = estimate_rust_nesting(source);
        assert!(estimate.depth <= 6, "{estimate:?}");
        assert!(estimate.else_if_chain <= 1, "{estimate:?}");
        assert!(rust_nesting_refusal(source).is_none());
        assert!(parse_clean_source_file(source).is_some());
    }

    #[test]
    fn deep_nesting_is_refused_before_parsing() {
        // Each shape aborted `ripr check` with a stack overflow in a debug
        // build at 600-1,000 levels before the budget existed.
        let cases = [
            nested("(", ")", 1_000),
            nested("{", "}", 1_000),
            nested("[", "]", 1_000),
            nested("if x > 1 { ", " } else { 0 }", 1_000),
            nested("(|y: i32| ", ")", 1_000),
            format!("pub fn f(x: i32) -> i32 {{ {}x }}\n", "- ".repeat(1_000)),
            format!("pub fn f(x: bool) -> bool {{ {}x }}\n", "!".repeat(1_000)),
            format!(
                "pub fn f(x: i32) -> i32 {{ if x == 0 {{ 0 }} {}else {{ 1 }} }}\n",
                (1..3_000)
                    .map(|i| format!("else if x == {i} {{ {i} }} "))
                    .collect::<String>()
            ),
            format!("pub fn f(x: i32) -> i32 {{ x{} }}\n", " + 1".repeat(5_000)),
            format!(
                "pub type T = {}u8{};\n",
                "Vec<".repeat(3_000),
                ">".repeat(3_000)
            ),
            format!(
                "pub fn f(x: i32) -> i32 {{ x{} }}\n",
                ".clone()".repeat(5_000)
            ),
            // Short runs per depth that chain through parens into one tree,
            // left-nested and right-nested.
            format!(
                "pub fn f(x: i32) -> i32 {{ {}x{} }}\n",
                "(".repeat(200),
                format!("){}", " + 1".repeat(40)).repeat(200)
            ),
            format!(
                "pub fn f(x: i32) -> i32 {{ {}x{} }}\n",
                format!("{}(", "1 + ".repeat(40)).repeat(200),
                ")".repeat(200)
            ),
            format!(
                "pub fn g(o: &S) -> Option<i32> {{ Some(o{}.v) }}\n",
                ".s.as_ref()?".repeat(20_000)
            ),
        ];
        for (case, source) in cases.iter().enumerate() {
            assert!(
                rust_nesting_refusal(source).is_some(),
                "case {case}: {:?}",
                estimate_rust_nesting(source)
            );
            assert!(parse_clean_source_file(source).is_none(), "case {case}");
        }
    }

    #[test]
    fn nesting_just_under_budget_still_parses() {
        let source = nested("(", ")", RUST_NESTING_BUDGET - 2);
        assert!(rust_nesting_refusal(&source).is_none());
        assert!(parse_clean_source_file(&source).is_some());
    }

    #[test]
    fn literal_contents_do_not_count() {
        let parens = "(".repeat(5_000);
        let source = format!(
            "pub const A: &str = \"{parens}\";\npub const B: &str = r##\"{parens}\"##;\n// {parens}\n/* {parens} */\n"
        );
        // Only the `&str` reference prefix counts.
        assert!(estimate_rust_nesting(&source).depth <= 1);
        assert!(parse_clean_source_file(&source).is_some());
    }

    #[test]
    fn statement_boundaries_reset_chains() {
        let statements: String = (0..5_000)
            .map(|i| format!("if x > {i} {{ return {i}; }}\n"))
            .collect();
        let source = format!("pub fn f(x: i32) -> i32 {{\n{statements}0 }}\n");
        let estimate = estimate_rust_nesting(&source);
        assert!(estimate.operator_run < 10, "{estimate:?}");
        assert!(rust_nesting_refusal(&source).is_none());

        // Ordinary field and method chains, floats, ranges and `?` reset per
        // statement too, so a long body of them stays within budget.
        let statements: String = (0..5_000)
            .map(|i| format!("let v{i} = self.a.b.get({i}..2).map(|x| x * 1.5)?;\n"))
            .collect();
        let source = format!("fn f(&self) -> Option<f64> {{\n{statements}None }}\n");
        let estimate = estimate_rust_nesting(&source);
        assert!(estimate.operator_run < 32, "{estimate:?}");
        assert!(rust_nesting_refusal(&source).is_none());
    }
}
