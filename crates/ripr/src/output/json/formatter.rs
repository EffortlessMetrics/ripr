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
    use super::escape;

    #[test]
    fn escapes_json() {
        assert_eq!(escape("a\"b\n"), "a\\\"b\\n");
    }
}
