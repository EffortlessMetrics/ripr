use super::{delimited_contents_at, enum_variant_values};

pub(in crate::analysis) fn exact_error_variant(text: &str) -> Option<String> {
    let start = text.find("Err(")?;
    let inner = delimited_contents_at(text, start + "Err".len())?;
    let inner = inner.trim();
    let outer_expression = inner
        .char_indices()
        .find(|(_, ch)| matches!(ch, '(' | '{' | '['))
        .map_or(inner, |(index, _)| &inner[..index])
        .trim();
    let values = enum_variant_values(outer_expression);
    (values.len() == 1).then(|| values[0].clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_error_variant_reads_first_variant_inside_result_error() {
        assert_eq!(
            exact_error_variant("return Err(AuthError::RevokedToken);").as_deref(),
            Some("AuthError::RevokedToken")
        );
    }

    #[test]
    fn exact_error_variant_returns_none_without_result_error() {
        assert_eq!(exact_error_variant("return Ok(value);"), None);
    }

    #[test]
    fn exact_error_variant_preserves_outer_nested_constructor() {
        assert_eq!(
            exact_error_variant("return Err(SomeError::Wrap(Inner::Value));").as_deref(),
            Some("SomeError::Wrap")
        );
    }
}
