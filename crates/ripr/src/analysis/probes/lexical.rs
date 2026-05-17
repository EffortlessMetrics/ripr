use crate::domain::ProbeFamily;

pub fn classify_changed_line(text: &str) -> Vec<ProbeFamily> {
    let text = text.trim_start();
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

fn has_predicate_shape(text: &str) -> bool {
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

fn has_call_shape(text: &str) -> bool {
    text.contains('(')
        && text.contains(')')
        && !is_function_signature(text)
        && !text.contains("assert")
}

fn has_field_shape(text: &str) -> bool {
    text.contains(':') && !text.contains("::") && !is_function_signature(text)
}

fn is_function_signature(text: &str) -> bool {
    text.starts_with("fn ") || text.starts_with("pub fn ")
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
        ] {
            let families = classify_changed_line(text);

            assert_eq!(
                families,
                vec![ProbeFamily::StaticUnknown],
                "{text} should stay static_unknown"
            );
        }
    }
}
