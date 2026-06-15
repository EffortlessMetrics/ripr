use crate::domain::{ProbeFamily, ProbeId, SymbolId};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Normalize an expression: trim leading/trailing whitespace and collapse
/// internal whitespace runs to a single space.
pub(crate) fn normalize_expression(expr: &str) -> String {
    let trimmed = expr.trim();
    let mut result = String::with_capacity(trimmed.len());
    let mut last_was_space = false;
    for ch in trimmed.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                result.push(' ');
            }
            last_was_space = true;
        } else {
            result.push(ch);
            last_was_space = false;
        }
    }
    result
}

/// Compute the 8-char hex fingerprint for a content-addressed probe id.
/// Payload is NUL-separated: `<sanitized_path>\0<family_str>\0<owner_str>\0<normalized_expression>\0`
fn compute_fp8(
    sanitized_path: &str,
    family_str: &str,
    owner_str: &str,
    normalized_expression: &str,
) -> String {
    // Normalize path separators in the owner symbol so the fingerprint is
    // platform-independent. The owner `SymbolId` embeds a file path walked
    // from the filesystem, which uses `\` on Windows and `/` elsewhere;
    // without this, the same code hashes to different ids per OS and goldens
    // blessed on one platform fail CI on another (#1053).
    let owner_normalized = owner_str.replace('\\', "/");
    let mut hasher = Sha256::new();
    hasher.update(sanitized_path.as_bytes());
    hasher.update(b"\0");
    hasher.update(family_str.as_bytes());
    hasher.update(b"\0");
    hasher.update(owner_normalized.as_bytes());
    hasher.update(b"\0");
    hasher.update(normalized_expression.as_bytes());
    hasher.update(b"\0");
    let hash = hasher.finalize();
    format!(
        "{:02x}{:02x}{:02x}{:02x}",
        hash[0], hash[1], hash[2], hash[3]
    )
}

/// Build a content-addressed probe id.
/// Format: `<prefix>:<sanitized_path>:<family_str>:<fp8>[.<ordinal>]`
/// The ordinal suffix (`.2`, `.3`, …) is OMITTED when ordinal == 1.
pub(crate) fn fingerprint_probe_id(
    prefix: &str,
    sanitized_path: &str,
    family_str: &str,
    owner_str: &str,
    normalized_expression: &str,
    ordinal: u32,
) -> ProbeId {
    let fp8 = compute_fp8(sanitized_path, family_str, owner_str, normalized_expression);
    if ordinal <= 1 {
        ProbeId(format!("{prefix}:{sanitized_path}:{family_str}:{fp8}"))
    } else {
        ProbeId(format!(
            "{prefix}:{sanitized_path}:{family_str}:{fp8}.{ordinal}"
        ))
    }
}

pub(crate) fn diff_probe_id(
    path: &Path,
    family: &ProbeFamily,
    owner: Option<&SymbolId>,
    expression: &str,
    ordinal: u32,
) -> ProbeId {
    let sp = sanitize_path(path);
    let family_str = family.as_str();
    let owner_str = owner.map(|o| o.0.as_str()).unwrap_or("");
    let norm = normalize_expression(expression);
    fingerprint_probe_id("probe", &sp, family_str, owner_str, &norm, ordinal)
}

pub(crate) fn repo_probe_id(
    path: &Path,
    family: &ProbeFamily,
    owner: Option<&SymbolId>,
    expression: &str,
    ordinal: u32,
) -> ProbeId {
    let sp = sanitize_path(path);
    let family_str = family.as_str();
    let owner_str = owner.map(|o| o.0.as_str()).unwrap_or("");
    let norm = normalize_expression(expression);
    fingerprint_probe_id("repo-probe", &sp, family_str, owner_str, &norm, ordinal)
}

pub(crate) fn sanitize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace(['/', '\\', ':'], "_")
        .trim_matches('_')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ProbeFamily;
    use std::path::PathBuf;

    #[test]
    fn sanitize_path_converts_separators_and_colons() {
        let path = PathBuf::from("src/lib.rs");
        let sanitized = sanitize_path(&path);
        assert_eq!(sanitized, "src_lib.rs");
    }

    #[test]
    fn sanitize_path_handles_windows_paths() {
        let path = PathBuf::from("workspace\\src\\lib.rs");
        let sanitized = sanitize_path(&path);
        assert_eq!(sanitized, "workspace_src_lib.rs");
    }

    #[test]
    fn sanitize_path_trims_underscores() {
        let path = PathBuf::from(":src/lib:");
        let sanitized = sanitize_path(&path);
        assert_eq!(sanitized, "src_lib");
    }

    #[test]
    fn content_addressed_id_stable_across_line_movement() {
        // Same (path, family, owner, expression) with two different lines → equal ids.
        let path = PathBuf::from("src/lib.rs");
        let family = ProbeFamily::Predicate;
        let owner = Some(SymbolId("my_module::my_fn".to_string()));
        let expression = "if x > 0 {";

        // ordinal 1 in both cases (different lines would previously produce different ids)
        let id_line3 = diff_probe_id(&path, &family, owner.as_ref(), expression, 1);
        let id_line99 = diff_probe_id(&path, &family, owner.as_ref(), expression, 1);

        assert_eq!(
            id_line3, id_line99,
            "ids must be identical regardless of line number"
        );
    }

    #[test]
    fn changed_expression_changes_id() {
        // Same (path, family, owner) but different expression → different ids.
        let path = PathBuf::from("src/lib.rs");
        let family = ProbeFamily::Predicate;
        let owner = Some(SymbolId("my_module::my_fn".to_string()));

        let id_gte = diff_probe_id(&path, &family, owner.as_ref(), "if a >= b {", 1);
        let id_gt = diff_probe_id(&path, &family, owner.as_ref(), "if a > b {", 1);

        assert_ne!(
            id_gte, id_gt,
            "changed expression must yield a different id"
        );
    }

    #[test]
    fn collision_suffix_appended_for_ordinal_2() {
        let path = PathBuf::from("src/lib.rs");
        let family = ProbeFamily::Predicate;
        let owner = Some(SymbolId("my_module::my_fn".to_string()));
        let expression = "if x > 0 {";

        let id1 = diff_probe_id(&path, &family, owner.as_ref(), expression, 1);
        let id2 = diff_probe_id(&path, &family, owner.as_ref(), expression, 2);

        // The id should NOT end with a `.N` collision suffix for ordinal 1.
        // (The id may contain '.' as part of the sanitized path like "lib.rs",
        //  so we check the fp8 hex segment does not have a dot after it.)
        assert!(
            !id1.0.ends_with(".1"),
            "ordinal 1 must not end with .1, got: {}",
            id1.0
        );
        // Verify the base ids are the same (same fingerprint), just different ordinal suffix.
        let base1 = id1.0.as_str();
        assert!(
            id2.0.ends_with(".2"),
            "ordinal 2 must end with .2, got: {}",
            id2.0
        );
        // The base (without .2) of ordinal-2 id should equal the ordinal-1 id.
        let base2 = id2.0.strip_suffix(".2").unwrap_or("");
        assert_eq!(
            base1, base2,
            "base ids must match; ordinal-1={base1}, ordinal-2={base2}"
        );
    }

    #[test]
    fn normalize_expression_collapses_whitespace() {
        assert_eq!(normalize_expression("  if   x > 0  {  "), "if x > 0 {");
        assert_eq!(normalize_expression("hello"), "hello");
        assert_eq!(normalize_expression(""), "");
    }
}
