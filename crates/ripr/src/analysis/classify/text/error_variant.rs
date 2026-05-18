use super::{delimited_contents_at, enum_variant_values};

pub(in crate::analysis) fn exact_error_variant(text: &str) -> Option<String> {
    let start = text.find("Err(")?;
    let inner = delimited_contents_at(text, start + "Err".len())?;
    enum_variant_values(&inner).into_iter().next()
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
}
