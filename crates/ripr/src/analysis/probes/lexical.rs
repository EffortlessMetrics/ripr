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
        && text.contains('(')
        && text.contains(')')
        && !is_function_signature(text)
        && !text.contains("assert")
        && !has_return_shape(text)
        && !starts_with_binding_or_control(text)
        && !text.trim_end().ends_with(',')
        && call_prefix_is_named(text)
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
