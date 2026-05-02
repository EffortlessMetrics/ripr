pub(crate) fn field(out: &mut String, indent: usize, name: &str, value: &str, trailing: bool) {
    out.push_str(&format!(
        "{}\"{}\": \"{}\"{}\n",
        "  ".repeat(indent),
        name,
        escape(value),
        if trailing { "," } else { "" }
    ));
}

pub(crate) fn number_field(
    out: &mut String,
    indent: usize,
    name: &str,
    value: usize,
    trailing: bool,
) {
    out.push_str(&format!(
        "{}\"{}\": {}{}\n",
        "  ".repeat(indent),
        name,
        value,
        if trailing { "," } else { "" }
    ));
}

pub(crate) fn float_field(out: &mut String, indent: usize, name: &str, value: f32, trailing: bool) {
    out.push_str(&format!(
        "{}\"{}\": {:.2}{}\n",
        "  ".repeat(indent),
        name,
        value,
        if trailing { "," } else { "" }
    ));
}

pub(crate) fn array_field(
    out: &mut String,
    indent: usize,
    name: &str,
    values: &[String],
    trailing: bool,
) {
    out.push_str(&format!("{}\"{}\": [", "  ".repeat(indent), name));
    for (idx, value) in values.iter().enumerate() {
        out.push_str(&format!("\"{}\"", escape(value)));
        if idx + 1 != values.len() {
            out.push_str(", ");
        }
    }
    out.push_str(&format!("]{}\n", if trailing { "," } else { "" }));
}

pub(crate) fn escape(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{array_field, escape, field, float_field, number_field};

    #[test]
    fn escapes_json() {
        assert_eq!(escape("a\"b\n"), "a\\\"b\\n");
    }

    #[test]
    fn escapes_backslash_and_control_chars() {
        assert_eq!(escape("\\\u{0008}\t"), "\\\\\\u0008\\t");
    }

    #[test]
    fn renders_scalar_fields() {
        let mut out = String::new();

        field(&mut out, 1, "name", "a\"b", true);
        number_field(&mut out, 1, "count", 7, true);
        float_field(&mut out, 1, "score", 0.125, false);

        assert_eq!(
            out,
            "  \"name\": \"a\\\"b\",\n  \"count\": 7,\n  \"score\": 0.12\n"
        );
    }

    #[test]
    fn renders_array_fields_with_and_without_values() {
        let mut out = String::new();

        array_field(
            &mut out,
            1,
            "stop_reasons",
            &["a\"b".to_string(), "c".to_string()],
            true,
        );
        array_field(&mut out, 1, "missing", &[], false);

        assert_eq!(
            out,
            "  \"stop_reasons\": [\"a\\\"b\", \"c\"],\n  \"missing\": []\n"
        );
    }
}
