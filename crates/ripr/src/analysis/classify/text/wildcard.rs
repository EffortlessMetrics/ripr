/// Whitespace-stable wildcard-discard binding predicate (#3233).
///
/// Recognizes `let` + the wildcard `_` + a `:`/`=` binding token across any
/// legal whitespace between the tokens: `let _ = expr`, `let _: Ty = expr`,
/// `let _ : Ty = expr`, and `let _=expr` are the same discard. The predicate
/// stays narrow to exactly that token grammar — an identifier continuation
/// (`let _value`, `let __x`) is a named binding, possibly used, and never a
/// discard; `let _;` has no discarded initializer.
///
/// Single authority for both classification stages: the infection stage's
/// discard detection and the flow stage's `value_is_swallowed` consume this
/// predicate so their contracts cannot drift apart again (#2401, #3233).
/// Rust lexical whitespace (Rust Reference: the Unicode
/// `Pattern_White_Space` set). `char::is_whitespace` differs from the Rust
/// lexer in both directions — NBSP (U+00A0) has the Unicode `White_Space`
/// property but is not Rust whitespace, while U+200E/U+200F are
/// `Pattern_White_Space` without the `White_Space` property — so the
/// predicate uses the lexer's set, not the general Unicode one.
fn is_rust_whitespace(ch: char) -> bool {
    matches!(
        ch,
        '\u{0009}'
            ..='\u{000D}'
                | '\u{0020}'
                | '\u{0085}'
                | '\u{200E}'
                | '\u{200F}'
                | '\u{2028}'
                | '\u{2029}'
    )
}

fn trim_rust_whitespace_start(text: &str) -> &str {
    text.trim_start_matches(is_rust_whitespace)
}

pub(in crate::analysis) fn is_wildcard_discard_binding(text: &str) -> bool {
    let Some(after_let) = trim_rust_whitespace_start(text).strip_prefix("let") else {
        return false;
    };
    // `let` must be a keyword, not the head of a longer identifier.
    if !after_let.starts_with(is_rust_whitespace) {
        return false;
    }
    let Some(after_wildcard) = trim_rust_whitespace_start(after_let).strip_prefix('_') else {
        return false;
    };
    // The character after `_` decides: whitespace or a binding token keeps
    // the wildcard interpretation; anything else is an identifier body.
    let binding = trim_rust_whitespace_start(after_wildcard);
    binding.starts_with('=') || binding.starts_with(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_wildcard_discards_across_whitespace_shapes() {
        for text in [
            "let _ = expr;",
            "let _: Ty = expr;",
            "let _ : Ty = expr;",
            "let _=expr;",
            "  let  _  =  expr;",
        ] {
            assert!(
                is_wildcard_discard_binding(text),
                "`{text}` must be a wildcard discard"
            );
        }
    }

    #[test]
    fn rejects_named_bindings_and_non_binding_wildcards() {
        for text in [
            "let _value = expr;",
            "let __x = expr;",
            "let _;",
            "let_thing = expr;",
            "let mut _ = expr;",
            "_ = expr;",
        ] {
            assert!(
                !is_wildcard_discard_binding(text),
                "`{text}` must not be a wildcard discard"
            );
        }
    }

    #[test]
    fn whitespace_is_the_rust_lexical_set_not_general_unicode() {
        // U+200E/U+200F are Pattern_White_Space (Rust lexical whitespace) but
        // lack the Unicode White_Space property; U+00A0 is the reverse. The
        // predicate follows the Rust lexer in both directions.
        assert!(is_wildcard_discard_binding("let\u{200E}_ = expr;"));
        assert!(is_wildcard_discard_binding("let\u{200F}_ = expr;"));
        assert!(!is_wildcard_discard_binding("let\u{00A0}_ = expr;"));
    }
}
