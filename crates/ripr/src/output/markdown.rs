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
    fn render_string_section_empty_values_renders_none_bullet() {
        let mut out = String::new();
        render_string_section(&mut out, "Example", &[]);
        assert!(out.contains("## Example"));
        assert!(out.contains("- none"));
    }

    #[test]
    fn render_string_section_escapes_backslashes() {
        let mut out = String::new();
        render_string_section(&mut out, "Example", &["a\\b".to_string()]);
        assert!(out.contains("- a\\\\b"));
    }

    #[test]
    fn markdown_text_escapes_backslashes() {
        assert_eq!(markdown_text("a\\b"), "a\\\\b");
        assert_eq!(markdown_text("no backslash"), "no backslash");
    }
}
