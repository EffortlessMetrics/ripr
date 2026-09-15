//! Shared test-body shadow detection for seam-callee identity.
//!
//! Both consumers — `analysis::classify::related_tests` (the
//! `SeamCalleeCall` test relation) and the guarded Result match scanner
//! (`analysis::extract::oracles::scan`) — must agree on when a same-named
//! local definition or binding in a test body impersonates a callee, so
//! the helpers live here as one authority (#3714, #3709). Callers pass a
//! comment-and-string-masked body (`mask_comments_and_strings`): masking
//! preserves byte layout, so body-relative line math stays exact against
//! the original text, and shadow-shaped text inside comments, strings, or
//! char literals never defeats a real call (#3728 rounds 3-5).
//!
//! #3727 Slice A adds a parser-backed decision path with the SAME rules at
//! line granularity: on files whose `FileFacts.used_lexical_fallback` is
//! false, [`ShadowAuthority::ParserBodyFacts`] derives the decision from
//! the parser-produced `nested_fn_names` / `let_bindings` fact fields; on
//! fallback files (or files absent from the index)
//! [`ShadowAuthority::LexicalMaskedBody`] runs the byte-level scanners
//! byte-identically. THE FLAG — not set emptiness — is the discriminator:
//! a parser-backed file can legitimately contain zero bindings, and empty
//! fact sets on parser-backed files are real "no shadow" results.

use crate::analysis::facts::LetBindingFact;

/// Which authority decides a test-body shadow for one scan input (#3727
/// Slice A). One authority per path, shared by both consumers, so the two
/// surfaces cannot disagree.
#[derive(Clone, Copy, Debug)]
pub(crate) enum ShadowAuthority<'a> {
    /// Run the byte-level lexical scanners over the comment-and-string
    /// masked body. Used when the scanned function's file has
    /// `used_lexical_fallback == true` or is absent from the index — the
    /// pre-#3727 behavior, byte-identical.
    LexicalMaskedBody,
    /// Derive the decision from parser-produced body facts
    /// (`FunctionFact`/`TestFact` `nested_fn_names` and `let_bindings`).
    /// Used only when the file's `used_lexical_fallback` is false. The
    /// masked body is ignored on this path: the facts were produced from
    /// real syntax, so comments and string contents never became facts.
    ParserBodyFacts {
        nested_fn_names: &'a [String],
        let_bindings: &'a [LetBindingFact],
    },
}

impl ShadowAuthority<'_> {
    /// Combined defeat at a use site, under this input's authority: a
    /// hoisted `fn <callee>` defeats every line; a `let` binding defeats at
    /// or after its own line. `body_line` is the body-relative line of the
    /// use (call site or match scrutinee). `masked_body` must be the
    /// comment-and-string-masked body for
    /// [`ShadowAuthority::LexicalMaskedBody`]; it is unused by
    /// [`ShadowAuthority::ParserBodyFacts`].
    pub(crate) fn body_shadows_callee_at_line(
        self,
        masked_body: &str,
        callee: &str,
        body_line: usize,
    ) -> bool {
        match self {
            Self::LexicalMaskedBody => body_shadows_callee_at_line(masked_body, callee, body_line),
            Self::ParserBodyFacts {
                nested_fn_names,
                let_bindings,
            } => fact_body_shadows_callee_at_line(nested_fn_names, let_bindings, callee, body_line),
        }
    }
}

/// Parser-fact twin of [`test_body_defines_callee_fn`] (#3727 Slice A): a
/// nested `fn <callee>` item is hoisted and defeats the whole body. The
/// producer records exact `ast::Fn` names, so whole-name equality carries
/// the lexical scanner's whole-word rule.
pub(crate) fn fact_body_defines_callee_fn(nested_fn_names: &[String], callee: &str) -> bool {
    !callee.is_empty() && nested_fn_names.iter().any(|name| name == callee)
}

/// Parser-fact twin of [`test_body_let_shadow_line`] (#3727 Slice A): the
/// body-relative line of the FIRST binding whose pattern names `callee`,
/// positional like the lexical scanner — a binding defeats uses at and
/// after its own line. `let_bindings` carries one entry per whole-word
/// pattern name (sorted by line, then name), so the earliest matching
/// entry is the scanner's first shadow.
pub(crate) fn fact_body_let_shadow_line(
    let_bindings: &[LetBindingFact],
    callee: &str,
) -> Option<usize> {
    if callee.is_empty() {
        return None;
    }
    let_bindings
        .iter()
        .filter(|binding| binding.name == callee)
        .map(|binding| binding.line)
        .min()
}

/// Parser-fact twin of [`body_shadows_callee_at_line`] (#3727 Slice A).
pub(crate) fn fact_body_shadows_callee_at_line(
    nested_fn_names: &[String],
    let_bindings: &[LetBindingFact],
    callee: &str,
    body_line: usize,
) -> bool {
    fact_body_defines_callee_fn(nested_fn_names, callee)
        || fact_body_let_shadow_line(let_bindings, callee).is_some_and(|shadow| shadow <= body_line)
}

/// Whole-word identifier extraction over one binding pattern's text
/// (#3727 Slice A): maximal runs of the exact character class the shared
/// [`pattern_contains_word`] matches against (ASCII alphanumeric plus
/// underscore). For ASCII pattern text — all realistic Rust patterns —
/// `extract_pattern_words(pattern).contains(callee)` is equivalent to
/// `pattern_contains_word(pattern, callee)`, so the fact-derived and
/// lexical decisions stay scanner-equivalent by construction. Residual
/// (documented): the byte-level scanner can see whole-word shapes in
/// non-ASCII identifier spellings this ASCII vocabulary does not tokenize;
/// such spellings never produce a matching fact name.
pub(crate) fn extract_pattern_words(pattern: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    for character in pattern.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            current.push(character);
        } else if !current.is_empty() {
            // Clone-and-clear, not `mem::take`: `take` leaves a
            // zero-capacity accumulator that reallocates on every word
            // (#3739 review, gemini h6ZRu). The output is identical.
            words.push(current.clone());
            current.clear();
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words.sort();
    words.dedup();
    words
}

/// #3714 round-2 review (devin hIL0i): the body-relative line index of the
/// FIRST `let` binding whose binding pattern names `callee` (bounded) — this
/// covers `let mut`, `let ref`, typed bindings, and destructuring patterns
/// (round-2 review, devin hDROE).
///
/// The caller passes a comment-and-string-masked body, so shadow-shaped text
/// inside comments or string literals never defeats a real call (#3728
/// round-3 review, coderabbit + devin).
///
/// A `let` binding only shadows uses at or after its own line, so the caller
/// defeats a captured call only when this shadow precedes it. `CallFact`
/// carries no column, so a call on the shadow's own line — e.g. a binding
/// initializer `let x = x(..)`, which resolves in the preceding scope — is
/// indistinguishable from a post-binding call on the same line and is
/// conservatively defeated (#3728 review; under-credit only,
/// column-precise attribution rides the same #3727 follow-up).
/// Nested-block bindings still defeat following calls in this
/// whole-body approximation (under-credit only: relations may be dropped,
/// never fabricated); full lexical scopes are the parser-backed follow-up
/// tracked on #3727.
pub(crate) fn test_body_let_shadow_line(body: &str, callee: &str) -> Option<usize> {
    if callee.is_empty() {
        return None;
    }
    let mut search = 0usize;
    while let Some(offset) = body[search..].find("let ") {
        let start = search + offset;
        search = start + 4;
        let before_ok = start == 0
            || !(body.as_bytes()[start - 1].is_ascii_alphanumeric()
                || body.as_bytes()[start - 1] == b'_');
        if !before_ok {
            continue;
        }
        // The binding pattern runs from the `let` to the initializer's
        // `=` (depth zero): `let mut x = ..`,
        // `let ref x = ..`, `let x: T = ..`, and
        // `let (a, x) = ..` are all covered by the pattern region. A
        // depth-zero `;` first means this declaration has no initializer
        // (`let flag;`): the scan must stop there rather than borrow a
        // LATER binding's `=`, which would move the reported shadow line
        // earlier and defeat calls between the two declarations
        // (#3728 round-5 review, devin).
        let region = &body[start + "let ".len()..];
        let bytes = region.as_bytes();
        let mut depth = 0isize;
        let mut in_string = false;
        let mut escaped = false;
        let mut pattern_end = None;
        for (offset, byte) in bytes.iter().enumerate() {
            if in_string {
                if escaped {
                    escaped = false;
                } else if *byte == b'\\' {
                    escaped = true;
                } else if *byte == b'"' {
                    in_string = false;
                }
                continue;
            }
            match byte {
                b'"' => in_string = true,
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => depth -= 1,
                b';' if depth == 0 => break,
                b'=' if depth == 0 => {
                    pattern_end = Some(offset);
                    break;
                }
                _ => {}
            }
        }
        let Some(pattern_end) = pattern_end else {
            continue;
        };
        // Whole-word containment only: a binding whose name merely BEGINS
        // with the callee (`let try_parse_summary_result = ..`) is a
        // different binding, not a shadow (#3714 round-2 review,
        // coderabbit).
        let pattern = region[..pattern_end].trim();
        if pattern_contains_word(pattern, callee) {
            let body_line = body[..start].bytes().filter(|byte| *byte == b'\n').count();
            return Some(body_line);
        }
    }
    None
}

/// #3714 round-1 review (devin hC): a same-named local definition in the
/// test body impersonates the seam callee — the captured `CallFact` name
/// alone cannot distinguish a real seam-callee call from a call of a
/// same-named helper defined in the test itself.
///
/// #3714 round-2 review (devin hIL0i): a `fn <callee>` item is hoisted and
/// in scope for the WHOLE test body, so one same-named local fn defeats
/// every captured call (unlike `let` bindings, which are positional — see
/// [`test_body_let_shadow_line`]).
///
/// The caller passes a comment-and-string-masked body, so a mentioned
/// `fn <callee>` shape inside a comment or string never defeats a real
/// call (#3728 round-3 review, coderabbit + devin).
///
/// Residual (documented): same-named definitions elsewhere in the test's
/// package and qualified paths remain indistinguishable at name level; the
/// `SeamCalleeCall` relation carries no variant claim, so the defeat gap can
/// only over-relate (weakly), never over-credit variant identity.
pub(crate) fn test_body_defines_callee_fn(body: &str, callee: &str) -> bool {
    if callee.is_empty() {
        return false;
    }
    let mut search = 0usize;
    while let Some(offset) = body[search..].find("fn ") {
        let start = search + offset;
        let before_ok = start == 0
            || !(body.as_bytes()[start - 1].is_ascii_alphanumeric()
                || body.as_bytes()[start - 1] == b'_');
        if before_ok {
            let name_start = start + "fn ".len();
            let name = body[name_start..].trim_start();
            if let Some(after_name) = name.strip_prefix(callee)
                && after_name
                    .chars()
                    .next()
                    .is_none_or(|ch| !(ch.is_ascii_alphanumeric() || ch == '_'))
            {
                return true;
            }
        }
        search = start + 3;
    }
    false
}

/// Combined defeat at a use site: a hoisted `fn <callee>` defeats every
/// line; a `let` binding defeats at or after its own line. `body_line` is
/// the body-relative line of the use (call site or match scrutinee).
/// Callers pass a comment-and-string-masked body.
pub(crate) fn body_shadows_callee_at_line(body: &str, callee: &str, body_line: usize) -> bool {
    test_body_defines_callee_fn(body, callee)
        || test_body_let_shadow_line(body, callee).is_some_and(|shadow| shadow <= body_line)
}

/// Whole-word containment: `word` delimited by non-identifier characters on
/// both sides.
fn pattern_contains_word(text: &str, word: &str) -> bool {
    let bytes = text.as_bytes();
    let mut search = 0usize;
    while let Some(offset) = text[search..].find(word) {
        let start = search + offset;
        let end = start + word.len();
        let before_ok =
            start == 0 || !(bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_');
        let after_ok =
            end >= bytes.len() || !(bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_');
        if before_ok && after_ok {
            return true;
        }
        search = end;
    }
    false
}

#[cfg(test)]
mod fact_authority_tests {
    use super::*;
    use crate::analysis::facts::LetBindingFact;

    fn binding(line: usize, name: &str) -> LetBindingFact {
        LetBindingFact {
            line,
            name: name.to_string(),
        }
    }

    #[test]
    fn fact_fn_defeat_requires_exact_name_and_nonempty_callee() {
        let nested = ["try_parse_summary".to_string(), "score".to_string()];
        assert!(fact_body_defines_callee_fn(&nested, "try_parse_summary"));
        assert!(fact_body_defines_callee_fn(&nested, "score"));
        assert!(
            !fact_body_defines_callee_fn(&nested, "try_parse_summar"),
            "prefix callee is a different name"
        );
        assert!(
            !fact_body_defines_callee_fn(&nested, "try_parse_summary_result"),
            "the nested fn name is not a prefix-match callee"
        );
        assert!(
            !fact_body_defines_callee_fn(&nested, "expect_response"),
            "unrelated name never defeats"
        );
        assert!(
            !fact_body_defines_callee_fn(&nested, ""),
            "empty callee defeats nothing"
        );
        assert!(!fact_body_defines_callee_fn(&[], "score"));
    }

    #[test]
    fn fact_let_shadow_is_first_matching_line_and_positional() {
        let bindings = vec![
            binding(0, "result"),
            binding(1, "try_parse_summary"),
            binding(1, "aux"),
            binding(4, "try_parse_summary"),
        ];
        assert_eq!(
            fact_body_let_shadow_line(&bindings, "try_parse_summary"),
            Some(1),
            "the FIRST matching binding line wins"
        );
        assert_eq!(fact_body_let_shadow_line(&bindings, "result"), Some(0));
        assert_eq!(fact_body_let_shadow_line(&bindings, "aux"), Some(1));
        assert_eq!(
            fact_body_let_shadow_line(&bindings, "expect_response"),
            None,
            "no matching binding means no shadow"
        );
        assert_eq!(
            fact_body_let_shadow_line(&bindings, ""),
            None,
            "empty callee never shadows"
        );
    }

    #[test]
    fn fact_combined_defeat_follows_the_positional_rules() {
        let nested = ["expect_response".to_string()];
        let bindings = vec![binding(3, "try_parse_summary")];
        assert!(fact_body_shadows_callee_at_line(
            &nested,
            &bindings,
            "expect_response",
            0
        ));
        assert!(!fact_body_shadows_callee_at_line(
            &nested,
            &bindings,
            "try_parse_summary",
            2
        ));
        assert!(fact_body_shadows_callee_at_line(
            &nested,
            &bindings,
            "try_parse_summary",
            3
        ));
        assert!(fact_body_shadows_callee_at_line(
            &nested,
            &bindings,
            "try_parse_summary",
            9
        ));
    }

    #[test]
    fn extract_pattern_words_matches_the_lexical_whole_word_class() {
        assert_eq!(
            extract_pattern_words("mut try_parse_summary"),
            vec!["mut".to_string(), "try_parse_summary".to_string()]
        );
        assert_eq!(
            extract_pattern_words("(a, x)"),
            vec!["a".to_string(), "x".to_string()]
        );
        assert_eq!(
            extract_pattern_words("Foo { x, y: z }"),
            vec![
                "Foo".to_string(),
                "x".to_string(),
                "y".to_string(),
                "z".to_string()
            ]
        );
        assert_eq!(extract_pattern_words("_"), vec!["_".to_string()]);
        assert_eq!(extract_pattern_words(""), Vec::<String>::new());
        // Every extracted word is a whole-word hit of the lexical
        // authority, and every whole-word hit is an extracted word.
        for (pattern, word) in [
            ("mut try_parse_summary", "try_parse_summary"),
            ("try_parse_summary_result", "try_parse_summary_result"),
            ("(a, x)", "x"),
            ("x1", "x1"),
        ] {
            assert!(
                extract_pattern_words(pattern)
                    .iter()
                    .any(|name| name == word),
                "`{word}` must tokenize out of `{pattern}`"
            );
        }
        for (pattern, word) in [
            ("try_parse_summary_result", "try_parse_summary"),
            ("x1", "x"),
            ("_x", "x"),
        ] {
            assert!(
                !extract_pattern_words(pattern)
                    .iter()
                    .any(|name| name == word),
                "`{word}` must NOT tokenize out of `{pattern}` (prefix/substring is another name)"
            );
        }
    }
}
