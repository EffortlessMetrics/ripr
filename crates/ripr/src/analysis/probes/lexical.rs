use crate::analysis::extract::mask_comments_and_strings;
use crate::domain::ProbeFamily;

pub fn classify_changed_line(text: &str) -> Vec<ProbeFamily> {
    let text = text.trim_start();
    // Masked once: comments between `pub` and `const` (`pub /* note */`)
    // must not hide a declaration, and string/comment contents must not
    // mint behavioral families downstream.
    let masked = mask_comments_and_strings(text);
    if is_constant_declaration(&masked) {
        return classify_constant_declaration(&masked);
    }
    let mut out = Vec::new();
    if has_predicate_shape(text) {
        out.push(ProbeFamily::Predicate);
    }
    if has_error_shape(text) {
        out.push(ProbeFamily::ErrorPath);
    }
    if has_return_shape(text) {
        out.push(ProbeFamily::ReturnValue);
    }
    if has_effect_shape(text) {
        out.push(ProbeFamily::SideEffect);
    }
    if has_call_shape(text) {
        out.push(ProbeFamily::CallDeletion);
    }
    if has_field_shape(text) {
        out.push(ProbeFamily::FieldConstruction);
    }
    if text.starts_with("match ") || text.contains("=>") {
        out.push(ProbeFamily::MatchArm);
    }
    if out.is_empty() {
        out.push(ProbeFamily::StaticUnknown);
    }
    out.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    out.dedup_by(|a, b| a.as_str() == b.as_str());
    out
}

/// Classify a constant (`const`/`static`) declaration line. `text` is the
/// string/comment-masked line, so literal data and comments never reach
/// the matchers.
///
/// FIX #3719: declaration syntax never reads as behavior — the
/// `pub(crate)` parens and `: Type` colon are gated absolutely, so no
/// `call_deletion` or `field_construction` family ever attaches, whatever
/// the initializer. Behavioral families come from the initializer span
/// only (masked text after the top-level `=`), so a type annotation such
/// as `NoneType` cannot mint `ReturnValue` while a genuine threshold
/// (`a > b`) still reads `Predicate`. `StaticUnknown` is always added
/// for the declared-flow limitation.
fn classify_constant_declaration(text: &str) -> Vec<ProbeFamily> {
    let scan = initializer_span(text);
    let mut out = Vec::new();
    if has_predicate_shape(scan) {
        out.push(ProbeFamily::Predicate);
    }
    if has_error_shape(scan) {
        out.push(ProbeFamily::ErrorPath);
    }
    if has_return_shape(scan) {
        out.push(ProbeFamily::ReturnValue);
    }
    if has_effect_shape(scan) {
        out.push(ProbeFamily::SideEffect);
    }
    if scan.starts_with("match ") || scan.contains("=>") {
        out.push(ProbeFamily::MatchArm);
    }
    out.push(ProbeFamily::StaticUnknown);
    out.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    out.dedup_by(|a, b| a.as_str() == b.as_str());
    out
}

/// Span of a constant initializer: masked text after the declaration's
/// top-level `=`. Depth-tracked so `==`, `=>`, `>=`, `<=`, `!=`, and
/// bracketed `=` never split. Falls back to the whole line when no
/// top-level `=` exists (same as matching the full line).
fn initializer_span(masked: &str) -> &str {
    let bytes = masked.as_bytes();
    let mut depth = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' if depth > 0 => depth -= 1,
            b'=' if depth == 0
                && bytes.get(index + 1) != Some(&b'=')
                && bytes.get(index + 1) != Some(&b'>')
                && (index == 0 || !matches!(bytes[index - 1], b'=' | b'!' | b'>' | b'<')) =>
            {
                return &masked[index + 1..];
            }
            _ => {}
        }
        index += 1;
    }
    masked
}

fn has_predicate_shape(text: &str) -> bool {
    // Guard against assertion-shaped probes (#2131): a line like
    // `debug_assert!(x > 5)` would match the ` > ` predicate token and be
    // misclassified as a Predicate probe. The guidance would then say
    // "Add boundary tests for below, equal, and above the changed threshold"
    // — meaningless advice for an assertion that has no threshold semantics.
    // Mirror the existing guard in has_call_shape (line 125).
    if is_assertion_macro(text) {
        return false;
    }
    text.contains(" if ")
        || text.starts_with("if ")
        || text.starts_with("while ")
        || text.contains(" >= ")
        || text.contains(" <= ")
        || text.contains(" > ")
        || text.contains(" < ")
        || text.contains(" == ")
        || text.contains(" != ")
        || text.contains("&&")
        || text.contains("||")
}

fn has_return_shape(text: &str) -> bool {
    text.starts_with("return ")
        || text.contains(" Ok(")
        || text.starts_with("Ok(")
        || text.contains(" Some(")
        || text.starts_with("Some(")
        || text.contains("None")
        || text.contains("return")
}

fn has_error_shape(text: &str) -> bool {
    text.contains("Err(")
        || text.contains("Error::")
        || text.contains("map_err")
        || text.contains("bail!")
        || text.contains("anyhow!")
        || contains_question_operator(text)
}

fn contains_question_operator(text: &str) -> bool {
    text.contains("?;")
        || text.contains("?.")
        || text.contains("?,")
        || text.contains("?)")
        || text.contains("? ")
        || text.ends_with('?')
}

fn has_effect_shape(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        ".save(",
        ".publish(",
        ".persist(",
        ".send(",
        ".dispatch(",
        ".notify(",
        ".enqueue(",
        ".write(",
        ".insert(",
        ".push(",
        ".remove(",
        ".delete(",
        ".emit(",
        ".increment(",
        ".replace(",
        ".clear(",
        ".extend(",
        ".store(",
        ".commit(",
        ".upsert(",
        ".configure(",
        ".set_option(",
        ".set_default(",
        ".set_var(",
        "config.",
        "settings.",
        "metrics.",
        "log::",
        "tracing::",
        "println!(",
        "eprintln!(",
        "trace!(",
        "debug!(",
        "info!(",
        "warn!(",
        "error!(",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// Whether the text begins with an assertion macro invocation (#2131).
/// These lines are never predicate probes — they are assertions that should
/// route to the default guidance ("Strengthen the related assertion so it
/// discriminates the changed behavior") rather than the predicate guidance
/// ("Add boundary tests for below, equal, and above the changed threshold").
fn is_assertion_macro(text: &str) -> bool {
    let trimmed = text.trim_start();
    [
        "assert!",
        "assert_eq!",
        "assert_ne!",
        "debug_assert!",
        "debug_assert_eq!",
        "debug_assert_ne!",
        "ensure!",
        "panic!",
        "unreachable!",
        "unimplemented!",
        "todo!",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix))
}

fn has_call_shape(text: &str) -> bool {
    !is_constant_declaration(text)
        && !is_tuple_type_declaration(text)
        && text.contains('(')
        && text.contains(')')
        && !is_function_signature(text)
        && !text.contains("assert")
        && !has_return_shape(text)
        && !starts_with_binding_or_control(text)
        && !text.trim_end().ends_with(',')
        && call_prefix_is_named(text)
}

/// Tuple enum variants and tuple structs are declarations, not executable
/// calls (#3740, #3749). `Invalid(String)`, `struct Wrap(PathBuf);`, and
/// `pub struct Wrapper(pub String);` must not become `call_deletion` probes.
/// Generics between the name and the tuple (`Foo<T>(pub T)`) belong to the
/// declaration. A same-line outer attribute (`#[derive(Clone)] struct ...`)
/// and a tuple-struct `where` tail are still the declaration. A value
/// argument (`NotFound(id)`, `Foo(value)`) and an expression statement
/// (`Invalid(msg);`) stay calls. `Err` / `Ok` / `Some` are constructors,
/// not variant declarations.
fn is_tuple_type_declaration(text: &str) -> bool {
    let mut rest = skip_outer_attributes(text.trim());
    if let Some(after_visibility) = strip_pub_visibility(rest) {
        rest = after_visibility.trim_start();
    }
    let struct_form = if let Some(after_struct) = rest.strip_prefix("struct ") {
        rest = after_struct.trim_start();
        true
    } else {
        false
    };
    let Some((name, after_name)) = take_rust_ident(rest) else {
        return false;
    };
    if matches!(name, "Err" | "Ok" | "Some") {
        return false;
    }
    if !name.starts_with(|ch: char| ch.is_ascii_uppercase()) {
        return false;
    }
    let mut after_name = after_name.trim_start();
    if after_name.starts_with('<') {
        let Some(skipped) = skip_balanced_generics(after_name) else {
            return false;
        };
        after_name = skipped.trim_start();
    }
    let Some(after_open) = after_name.strip_prefix('(') else {
        return false;
    };
    let Some((inner, tail)) = split_matching_paren(after_open) else {
        return false;
    };
    if !type_argument_list(inner) {
        return false;
    }
    let tail = tail.trim();
    if struct_form {
        return tail.is_empty() || tail == ";" || is_where_clause(tail);
    }
    if tail.is_empty() || tail == "," || tail == "}" || tail == "}," {
        return true;
    }
    numeric_discriminant(tail)
}

/// `#[...]` / `#![...]` prefixes on the same line as a declaration.
/// A line that is not an attribute is returned unchanged.
fn skip_outer_attributes(text: &str) -> &str {
    let mut rest = text.trim_start();
    loop {
        let Some(after_hash) = rest.strip_prefix('#') else {
            return rest;
        };
        let after_inner = after_hash.strip_prefix('!').unwrap_or(after_hash);
        let Some(inside) = after_inner.trim_start().strip_prefix('[') else {
            return rest;
        };
        let Some(after_attr) = skip_balanced_delims(inside, '[', ']') else {
            return rest;
        };
        rest = after_attr.trim_start();
    }
}

fn skip_balanced_delims(text: &str, open: char, close: char) -> Option<&str> {
    let mut depth = 1usize;
    for (index, ch) in text.char_indices() {
        if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(&text[index + ch.len_utf8()..]);
            }
        }
    }
    None
}

fn is_where_clause(tail: &str) -> bool {
    let Some(after) = tail.strip_prefix("where") else {
        return false;
    };
    // A `where` bound needs no whitespace after the keyword (`where'a: 'b`,
    // `where[T; 4]: Clone`). Only a trailing identifier character means the
    // line names something else (`whereabouts`, `where_clause`).
    after.is_empty() || after.starts_with(|ch: char| !ch.is_alphanumeric() && ch != '_')
}

fn numeric_discriminant(tail: &str) -> bool {
    let Some(after_eq) = tail.strip_prefix('=') else {
        return false;
    };
    let mut text = after_eq.trim();
    if let Some(without_comma) = text.strip_suffix(',') {
        text = without_comma.trim();
    }
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

fn strip_pub_visibility(text: &str) -> Option<&str> {
    let rest = text.trim_start();
    let after_pub = rest.strip_prefix("pub")?;
    // Do not trim before the word-boundary check. `pub struct` has a space
    // after `pub`; trimming it makes `struct` look like an identifier suffix.
    if let Some(after_paren) = after_pub.strip_prefix('(') {
        let (_, after_visibility) = split_matching_paren(after_paren)?;
        return Some(after_visibility);
    }
    if after_pub.is_empty() || after_pub.starts_with(|ch: char| ch.is_whitespace()) {
        return Some(after_pub);
    }
    None
}

fn take_rust_ident(text: &str) -> Option<(&str, &str)> {
    let mut end = 0usize;
    for (index, ch) in text.char_indices() {
        let ok = if index == 0 {
            ch == '_' || ch.is_ascii_alphabetic()
        } else {
            ch == '_' || ch.is_ascii_alphanumeric()
        };
        if !ok {
            break;
        }
        end = index + ch.len_utf8();
    }
    if end == 0 {
        None
    } else {
        Some((&text[..end], &text[end..]))
    }
}

fn split_matching_paren(text: &str) -> Option<(&str, &str)> {
    let mut depth = 1usize;
    for (index, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&text[..index], &text[index + ch.len_utf8()..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// Text after a `<...>` generic argument list, or `None` when the brackets
/// do not close. Nested `Foo<Bar<T>>` stays inside the declaration name.
fn skip_balanced_generics(text: &str) -> Option<&str> {
    let inner = text.trim_start().strip_prefix('<')?;
    let mut depth = 1usize;
    for (index, ch) in inner.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&inner[index + ch.len_utf8()..]);
                }
            }
            _ => {}
        }
    }
    None
}

const TYPE_ARGUMENT_WORDS: &[&str] = &[
    "str", "bool", "char", "dyn", "mut", "const", "i8", "i16", "i32", "i64", "i128", "isize", "u8",
    "u16", "u32", "u64", "u128", "usize", "f32", "f64",
];

fn type_argument_list(inner: &str) -> bool {
    let text = inner.trim();
    if text.is_empty() {
        return false;
    }
    let bytes = text.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte.is_ascii_whitespace()
            || matches!(
                byte,
                b'&' | b',' | b'<' | b'>' | b'[' | b']' | b'(' | b')' | b'*'
            )
        {
            index += 1;
            continue;
        }
        if byte == b':' {
            if bytes.get(index + 1) == Some(&b':') {
                index += 2;
                continue;
            }
            return false;
        }
        if byte == b'\'' {
            index += 1;
            let Some((ident, _)) = take_rust_ident(&text[index..]) else {
                return false;
            };
            index += ident.len();
            continue;
        }
        if byte.is_ascii_alphabetic() || byte == b'_' {
            let Some((ident, _)) = take_rust_ident(&text[index..]) else {
                return false;
            };
            if ident == "pub" {
                let Some(after_vis) = skip_field_visibility(&text[index..]) else {
                    return false;
                };
                index = text.len() - after_vis.len();
                continue;
            }
            if ident.starts_with(|ch: char| ch.is_ascii_lowercase())
                && !TYPE_ARGUMENT_WORDS.contains(&ident)
            {
                let rest = text[index + ident.len()..].trim_start();
                if !rest.starts_with("::") {
                    return false;
                }
            }
            index += ident.len();
            continue;
        }
        return false;
    }
    true
}

/// Remainder after tuple-field visibility (`pub` or `pub(...)`).
///
/// `pub String` and `pub(crate) u32` are types. A bare `pub` is not, so
/// `Foo(pub)` stays a value argument.
fn skip_field_visibility(text: &str) -> Option<&str> {
    let after_pub = text.strip_prefix("pub")?;
    let trimmed = after_pub.trim_start();
    if let Some(after_open) = trimmed.strip_prefix('(') {
        let (_, after_vis) = split_matching_paren(after_open)?;
        return Some(after_vis);
    }
    if trimmed.starts_with(|ch: char| {
        ch.is_ascii_alphabetic() || matches!(ch, '_' | '&' | '\'' | '[' | '*')
    }) {
        return Some(after_pub);
    }
    None
}

fn starts_with_binding_or_control(text: &str) -> bool {
    ["let ", "if ", "while ", "for ", "match "]
        .iter()
        .any(|prefix| text.starts_with(prefix))
}

fn call_prefix_is_named(text: &str) -> bool {
    text.split_once('(')
        .map(|(prefix, _)| {
            prefix
                .trim_end()
                .chars()
                .next_back()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '!'))
        })
        .unwrap_or(false)
}

fn has_field_shape(text: &str) -> bool {
    !is_constant_declaration(text)
        && !is_tuple_type_declaration(text)
        && text.contains(':')
        && !text.contains("::")
        && !is_function_signature(text)
}

fn is_function_signature(text: &str) -> bool {
    let mut rest = text.trim_start();

    if let Some(next) = rest.strip_prefix("pub ") {
        rest = next.trim_start();
    } else if let Some(next) = rest.strip_prefix("pub(") {
        let Some((_, after_visibility)) = next.split_once(')') else {
            return false;
        };
        rest = after_visibility.trim_start();
    }

    loop {
        if let Some(next) = rest.strip_prefix("async ") {
            rest = next.trim_start();
        } else if let Some(next) = rest.strip_prefix("const ") {
            rest = next.trim_start();
        } else if let Some(next) = rest.strip_prefix("unsafe ") {
            rest = next.trim_start();
        } else if let Some(next) = rest.strip_prefix("extern ") {
            rest = strip_extern_abi(next).trim_start();
        } else {
            break;
        }
    }

    rest.starts_with("fn ")
}

fn strip_extern_abi(text: &str) -> &str {
    let text = text.trim_start();
    let Some(rest) = text.strip_prefix('"') else {
        return text;
    };
    let Some((_abi, after_quote)) = rest.split_once('"') else {
        return text;
    };
    after_quote
}

/// FIX #3719: a `const`/`static` declaration (with any visibility, including
/// `pub(crate)`) is a named value binding — its `pub(crate)` parentheses and
/// `: Type` colon are declaration syntax, not call or field-construction
/// behavior. Without this gate, `pub(crate) const X: u32 = 3;` misclassified
/// as both call_deletion and field_construction (#3719).
fn is_constant_declaration(text: &str) -> bool {
    let mut rest = text.trim_start();

    if let Some(next) = rest.strip_prefix("pub") {
        // Tolerates `pub(crate)`, `pub (crate)`, and bare `pub const`.
        let after_pub = next.trim_start();
        if let Some(rest_after_vis) = after_pub.strip_prefix('(') {
            let Some((_, after_visibility)) = rest_after_vis.split_once(')') else {
                return false;
            };
            rest = after_visibility.trim_start();
        } else {
            rest = after_pub;
        }
    }

    rest.starts_with("const ") || rest.starts_with("static ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_changed_line_detects_core_probe_shapes() {
        let cases = [
            ("if x > 5 { }", ProbeFamily::Predicate),
            ("return Ok(total)", ProbeFamily::ReturnValue),
            ("Err(AuthError::Revoked)", ProbeFamily::ErrorPath),
            ("events.publish(invoice)", ProbeFamily::SideEffect),
            ("send_invoice(invoice)", ProbeFamily::CallDeletion),
            ("total: discounted_total", ProbeFamily::FieldConstruction),
            ("match status {", ProbeFamily::MatchArm),
            ("Status::Ready => total", ProbeFamily::MatchArm),
            ("let value = total;", ProbeFamily::StaticUnknown),
        ];

        for (text, expected) in cases {
            let families = classify_changed_line(text);
            assert!(
                families.contains(&expected),
                "{text} did not classify as {}",
                expected.as_str()
            );
        }
    }

    /// FIX #3719: a constant declaration must not acquire call_deletion or
    /// field_construction behavior from its `pub(crate)` parentheses or its
    /// `: Type` colon. It falls through to StaticUnknown, which the canonical
    /// alignment maps to the static_limitation canonical item.
    #[test]
    fn constant_declarations_do_not_acquire_call_or_field_shapes() {
        for text in [
            "pub(crate) const OBSERVATION_SCHEMA_GENERATION: u32 = 3;",
            "pub const MAX_RETRIES: usize = 5;",
            "const CACHE_TTL_SECS: u64 = 60;",
            "static ACTIVE: AtomicUsize = AtomicUsize::new(0);",
            "pub static INSTANCE: OnceLock<Config> = OnceLock::new();",
        ] {
            let families = classify_changed_line(text);
            assert!(
                !families.contains(&ProbeFamily::CallDeletion),
                "{text} must not classify as call_deletion"
            );
            assert!(
                !families.contains(&ProbeFamily::FieldConstruction),
                "{text} must not classify as field_construction"
            );
            assert!(
                families.contains(&ProbeFamily::StaticUnknown),
                "{text} should fall through to static_unknown"
            );
        }
    }

    /// Review (#3720, devin BUG thread): declarations with behavioral-shape
    /// initializers keep the initializer family alongside StaticUnknown —
    /// the threshold comparison still reads Predicate, the Err initializer
    /// still reads ErrorPath — while declaration syntax never reads as
    /// call/field behavior.
    #[test]
    fn constant_declarations_keep_initializer_behavior_alongside_unknown() {
        for (text, expected) in [
            (
                "const VALUE: Result<(), Error> = Err(error);",
                ProbeFamily::ErrorPath,
            ),
            (
                "pub(crate) const READY: bool = a > b;",
                ProbeFamily::Predicate,
            ),
        ] {
            let families = classify_changed_line(text);
            assert!(
                families.contains(&expected),
                "{text} must keep {} from its initializer",
                expected.as_str()
            );
            assert!(
                families.contains(&ProbeFamily::StaticUnknown),
                "{text} must record the declared-flow limitation"
            );
            assert!(
                !families.contains(&ProbeFamily::CallDeletion),
                "{text} must not read declaration syntax as call_deletion"
            );
            assert!(
                !families.contains(&ProbeFamily::FieldConstruction),
                "{text} must not read declaration syntax as field_construction"
            );
        }
    }

    /// Review (#3720, devin BUG thread): operator/match tokens inside
    /// string literals are data, not behavior — they must not mint
    /// threshold or match families on a declaration line.
    #[test]
    fn constant_string_literals_do_not_mint_behavioral_families() {
        for text in [
            "const OPERATOR: &str = \" > \";",
            "const ARROW: &str = \"=>\";",
            "pub(crate) const MSG: &str = \"Err(not real)\";",
        ] {
            let families = classify_changed_line(text);
            assert_eq!(
                families,
                vec![ProbeFamily::StaticUnknown],
                "{text} must classify as static_unknown alone"
            );
        }
    }

    /// Review (#3720, coderabbit Major threads): comments between `pub`
    /// and `const` must not hide the declaration, and the type annotation
    /// must not mint behavioral families — only the initializer span can.
    #[test]
    fn constant_declaration_gate_ignores_comments_and_annotations() {
        for text in [
            "pub /* note */ const VALUE: u32 = 3;",
            "const VALUE: NoneType = value;",
        ] {
            let families = classify_changed_line(text);
            assert_eq!(
                families,
                vec![ProbeFamily::StaticUnknown],
                "{text} must classify as static_unknown alone"
            );
        }
    }

    /// Controls: actual deletable calls and actual field constructions keep
    /// their families, and a const with a function-call initializer stays
    /// out of call_deletion (the initializer call is not the declaration).
    #[test]
    fn call_and_field_controls_keep_their_families() {
        assert!(
            classify_changed_line("send_invoice(invoice)").contains(&ProbeFamily::CallDeletion)
        );
        assert!(
            classify_changed_line("total: discounted_total")
                .contains(&ProbeFamily::FieldConstruction)
        );
        let initializer =
            classify_changed_line("pub(crate) const LIMIT: usize = compute_limit(64);");
        assert!(
            !initializer.contains(&ProbeFamily::CallDeletion),
            "const initializer call must not classify the declaration as call_deletion"
        );
    }

    /// #3740 / #3749: tuple enum variants and tuple structs are declarations.
    /// The reported `Invalid(String),` lines already fail the trailing-comma
    /// gate; the last variant (no comma) and a tuple struct still matched
    /// `call_deletion`. Field visibility and generics are part of the
    /// declaration. Value arguments and `Err(...)` stay calls.
    #[test]
    fn tuple_type_declarations_are_not_call_deletion() {
        for text in [
            "Invalid(String),",
            "Ambiguous(String),",
            "Invalid(String)",
            "pub(crate) Invalid(&str)",
            "Invalid(Box<String>)",
            "Invalid(std::path::PathBuf) = 1",
            "struct Wrap(String);",
            "pub struct Wrap(std::path::PathBuf);",
            "struct Foo(String);",
            "pub struct Foo(pub String);",
            "pub(crate) struct Foo(pub(crate) u32);",
            "pub struct Foo<T>(pub T);",
            "struct Foo<'a>(&'a str);",
            "struct Foo<T: Clone>(T);",
            "#[derive(Clone)] struct Wrap(String);",
            "#[repr(transparent)] pub struct Wrap(String);",
            "#[derive(Debug)] #[repr(transparent)] pub struct Wrap(u32);",
            "#[derive(Debug)] Invalid(String),",
            "struct Foo<T>(T) where T: Clone;",
            "pub struct Foo<T>(pub T) where T: Clone;",
            "#[derive(Clone)] pub struct Foo<T>(T) where T: Clone;",
            "struct Foo<'a, 'b>(&'a str) where'a: 'b;",
            "struct Foo<T>(T) where[T; 4]: Clone;",
        ] {
            let families = classify_changed_line(text);
            assert_eq!(
                families,
                vec![ProbeFamily::StaticUnknown],
                "{text} must be a non-executable declaration, got {families:?}"
            );
        }
        for text in [
            "send_invoice(invoice)",
            "NotFound(id)",
            "Invalid(msg);",
            "Err(AuthError::Revoked)",
            "Foo(value)",
            "Id(0)",
            "#[inline] send_invoice(invoice)",
            "#[allow(unused)] Foo(value)",
        ] {
            let families = classify_changed_line(text);
            assert!(
                families.contains(&ProbeFamily::CallDeletion),
                "{text} must stay call_deletion, got {families:?}"
            );
        }
    }

    #[test]
    fn classify_changed_line_detects_fallible_error_context() {
        let families = classify_changed_line("let parsed = parse()?; ErrKind::Invalid");

        assert!(families.contains(&ProbeFamily::ErrorPath));
    }

    #[test]
    fn classify_changed_line_detects_bare_question_operator() {
        for text in [
            "let x = func()?;",
            "stream.read_to_end(&mut buf)?;",
            "let value = parse()?.trim().to_string();",
        ] {
            let families = classify_changed_line(text);

            assert!(
                families.contains(&ProbeFamily::ErrorPath),
                "{text} did not classify as error_path"
            );
        }
    }

    #[test]
    fn classify_changed_line_detects_observable_effect_families() {
        for text in [
            "events.publish(invoice)",
            "cache.insert(key, value)",
            "repository.save(invoice)",
            "log::info!(\"saved\")",
            "config.set_option(\"mode\", mode)",
        ] {
            let families = classify_changed_line(text);

            assert!(
                families.contains(&ProbeFamily::SideEffect),
                "{text} did not classify as side_effect"
            );
        }
    }

    #[test]
    fn classify_changed_line_handles_indented_rust_shapes() {
        let cases = [
            ("    while ready {", ProbeFamily::Predicate),
            ("        return None;", ProbeFamily::ReturnValue),
            ("        match status {", ProbeFamily::MatchArm),
        ];

        for (text, expected) in cases {
            let families = classify_changed_line(text);

            assert!(
                families.contains(&expected),
                "{text} did not classify as {}",
                expected.as_str()
            );
        }
    }

    #[test]
    fn classify_changed_line_does_not_treat_indented_function_signatures_as_probes() {
        for text in [
            "    fn helper(value: usize) -> usize {",
            "    pub fn helper(value: usize) -> usize {",
            "    pub(crate) fn helper(value: usize) -> usize {",
            "    async fn helper(value: usize) -> usize {",
            "    pub async fn helper(value: usize) -> usize {",
            "    pub(crate) unsafe extern \"C\" fn helper(value: usize) -> usize {",
        ] {
            let families = classify_changed_line(text);

            assert_eq!(
                families,
                vec![ProbeFamily::StaticUnknown],
                "{text} should stay static_unknown"
            );
        }
    }

    #[test]
    fn classify_changed_line_rejects_non_standalone_call_shapes() -> Result<(), String> {
        for text in [
            "let value = read()?;",
            "let Some(value) = selected else { return None; };",
            "if let Some(value) = selected {",
            "(GateState::Pending, None)",
            "Ok(())",
            "Vec::new(),",
        ] {
            let families = classify_changed_line(text);
            if families.contains(&ProbeFamily::CallDeletion) {
                return Err(format!("{text} should not be a call-deletion probe"));
            }
        }
        Ok(())
    }
}
