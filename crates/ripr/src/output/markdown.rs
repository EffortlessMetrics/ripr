pub(crate) fn render_string_section(out: &mut String, title: &str, values: &[String]) {
    out.push_str(&format!("\n## {title}\n\n"));
    if values.is_empty() {
        out.push_str("- none\n");
    } else {
        for value in values {
            out.push_str(&format!("- {}\n", markdown_text(value)));
        }
    }
}

pub(crate) fn markdown_text(value: &str) -> String {
    value.replace('\\', "\\\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_text_escapes_backslashes() {
        assert_eq!(markdown_text("a\\b"), "a\\\\b");
        assert_eq!(markdown_text("no backslash"), "no backslash");
    }

    #[test]
    fn render_string_section_lists_values_or_none() {
        let mut out = String::new();
        render_string_section(&mut out, "Example", &[]);
        assert_eq!(out, "\n## Example\n\n- none\n");

        let mut out = String::new();
        render_string_section(&mut out, "Example", &["a\\b".to_string()]);
        assert_eq!(out, "\n## Example\n\n- a\\\\b\n");
    }
}
