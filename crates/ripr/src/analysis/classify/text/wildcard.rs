/// Return whether `text` is a bounded Rust wildcard-discard binding.
pub(in crate::analysis) fn is_wildcard_discard_binding(text: &str) -> bool {
    let trimmed = text.trim_start();
    let Some(rest) = trimmed.strip_prefix("let") else {
        return false;
    };
    let Some(first) = rest.chars().next() else {
        return false;
    };
    if !first.is_ascii_whitespace() {
        return false;
    }
    let rest = rest.trim_start();
    let Some(rest) = rest.strip_prefix('_') else {
        return false;
    };
    let rest = rest.trim_start();
    if let Some(initializer) = rest.strip_prefix('=') {
        return !initializer.trim().is_empty();
    }
    let Some(rest) = rest.strip_prefix(':') else {
        return false;
    };
    let Some((ty, initializer)) = rest.split_once('=') else {
        return false;
    };
    !ty.trim().is_empty() && !initializer.trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::is_wildcard_discard_binding;

    #[test]
    fn recognizes_bounded_whitespace_variants() {
        for text in [
            "let _ = value",
            "let _: Ty = value",
            "let _ : Ty = value",
            "let   _   : Ty = value",
            "let _= value",
        ] {
            assert!(is_wildcard_discard_binding(text), "{text}");
        }
    }

    #[test]
    fn rejects_non_discard_bindings_and_incomplete_forms() {
        for text in [
            "let _value = value",
            "let __x = value",
            "let _",
            "let_thing = value",
            "let mut _ = value",
            "_ = value",
            "let (a, _) = value",
            "let _ : Ty",
            "let _ =",
        ] {
            assert!(!is_wildcard_discard_binding(text), "{text}");
        }
    }
}
