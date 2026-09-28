//! Commit-record parsing shared by `build.rs` and the crate's unit tests.
//!
//! `build.rs` includes this file with `#[path]`, and the crate compiles it
//! only under `cfg(test)`, so it must stay free of `crate::` paths and
//! dependencies.

/// Whether `value` is a full Git object id: 40 (SHA-1) or 64 (SHA-256)
/// lowercase hex digits. Abbreviated or decorated ids are not an identity.
pub(crate) fn is_full_commit_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// The commit id and dirty flag that `cargo package` records in the
/// `.cargo_vcs_info.json` of every packaged crate:
///
/// ```json
/// {"git": {"sha1": "<40 hex>", "dirty": true}, "path_in_vcs": "crates/ripr"}
/// ```
///
/// `dirty` is present only when the package was made with `--allow-dirty`
/// from a modified tree. A record without a full commit id yields `None`.
pub(crate) fn parse_cargo_vcs_info(text: &str) -> Option<(String, bool)> {
    let sha1 = json_value_after_key(text, "sha1")?
        .strip_prefix('"')?
        .split('"')
        .next()?;
    if !is_full_commit_id(sha1) {
        return None;
    }
    let dirty = json_value_after_key(text, "dirty").is_some_and(|value| value.starts_with("true"));
    Some((sha1.to_string(), dirty))
}

/// The text after `"key"` and its `:` separator, with leading whitespace
/// removed.
fn json_value_after_key<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let quoted = format!("\"{key}\"");
    let (_, rest) = text.split_once(quoted.as_str())?;
    rest.trim_start().strip_prefix(':').map(str::trim_start)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "e4da2d4a0c1b2d3e4f5a6b7c8d9e0f1a2b3c4d5e";

    #[test]
    fn cargo_vcs_info_yields_clean_commit() {
        let text = format!(
            "{{\n  \"git\": {{\n    \"sha1\": \"{SHA}\"\n  }},\n  \"path_in_vcs\": \"crates/ripr\"\n}}"
        );
        assert_eq!(parse_cargo_vcs_info(&text), Some((SHA.to_string(), false)));
    }

    #[test]
    fn cargo_vcs_info_carries_dirty_flag() {
        let text =
            format!("{{\"git\":{{\"sha1\":\"{SHA}\",\"dirty\":true}},\"path_in_vcs\":\"\"}}");
        assert_eq!(parse_cargo_vcs_info(&text), Some((SHA.to_string(), true)));
        let clean = text.replace("true", "false");
        assert_eq!(parse_cargo_vcs_info(&clean), Some((SHA.to_string(), false)));
    }

    #[test]
    fn cargo_vcs_info_without_full_commit_is_unknown() {
        for text in [
            "",
            "{\"git\":{}}",
            "{\"git\":{\"sha1\":\"e4da2d4\"}}",
            "{\"git\":{\"sha1\":\"E4DA2D4A0C1B2D3E4F5A6B7C8D9E0F1A2B3C4D5E\"}}",
            "{\"git\":{\"sha1\":null}}",
        ] {
            assert_eq!(parse_cargo_vcs_info(text), None, "{text:?}");
        }
    }

    #[test]
    fn full_commit_ids_are_forty_or_sixty_four_lowercase_hex() {
        assert!(is_full_commit_id(SHA));
        assert!(is_full_commit_id(&"a".repeat(64)));
        assert!(!is_full_commit_id(&SHA[..39]));
        assert!(!is_full_commit_id(&format!("{SHA}0")));
        assert!(!is_full_commit_id(&SHA.replace('e', "g")));
    }
}
