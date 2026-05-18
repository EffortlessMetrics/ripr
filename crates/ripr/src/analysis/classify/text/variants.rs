pub(in crate::analysis) fn enum_variant_values(text: &str) -> Vec<String> {
    let mut values = Vec::new();
    for token in text.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')) {
        if !token.contains("::") {
            continue;
        }
        let Some(last) = token.rsplit("::").next() else {
            continue;
        };
        if last
            .chars()
            .next()
            .is_some_and(|ch| ch.is_ascii_uppercase())
        {
            values.push(token.to_string());
        }
    }
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enum_variant_values_returns_sorted_unique_variants() {
        let values = enum_variant_values(
            "Err(AuthError::RevokedToken) Err(AuthError::ExpiredToken) AuthError::RevokedToken",
        );

        assert_eq!(
            values,
            vec![
                "AuthError::ExpiredToken".to_string(),
                "AuthError::RevokedToken".to_string()
            ]
        );
    }

    #[test]
    fn enum_variant_values_ignores_lowercase_and_unqualified_tokens() {
        assert_eq!(
            enum_variant_values("err(auth_error::revoked) Revoked"),
            Vec::<String>::new()
        );
    }
}
