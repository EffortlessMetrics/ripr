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
    fn escapes_control_chars_as_unicode() {
        assert_eq!(escape("ok\u{0001}end"), "ok\\u0001end");
    }

    #[test]
    fn writes_string_and_numeric_fields() {
        let mut out = String::new();
        field(&mut out, 1, "name", "a\tb", true);
        number_field(&mut out, 1, "count", 3, true);
        float_field(&mut out, 1, "ratio", 1.234, false);

        assert_eq!(
            out,
            "  \"name\": \"a\\tb\",\n  \"count\": 3,\n  \"ratio\": 1.23\n"
        );
    }

    #[test]
    fn writes_array_field_with_and_without_values() {
        let mut out = String::new();
        array_field(
            &mut out,
            0,
            "items",
            &["a\"b".to_string(), "c".to_string()],
            true,
        );
        array_field(&mut out, 0, "empty", &[], false);

        assert_eq!(out, "\"items\": [\"a\\\"b\", \"c\"],\n\"empty\": []\n");
    }
}
