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

/// The id ripr 0.10 gave a Rust diff finding: the same fingerprint over the
/// whole trimmed changed line instead of the parser-canonical expression
/// (#4736). Used only to name the replacement for a stale suppression; it
/// never matches or suppresses anything by itself.
/// `source` is the current text of `probe.location.file`; the caller reads
/// it once per file.
pub(crate) fn legacy_whole_line_diff_probe_id(
    root: &Path,
    probe: &crate::domain::Probe,
    source: &str,
) -> Option<String> {
    let relative = probe.location.file.strip_prefix(root).ok()?;
    if relative.extension().and_then(|ext| ext.to_str()) != Some("rs") {
        return None;
    }
    let line = source.lines().nth(probe.location.line.checked_sub(1)?)?;
    // 0.10 suffixed repeated ids (`.2`, `.3`) in diff order. When the same
    // line text appears more than once in the file, which occurrence got the
    // bare id cannot be recovered from one finding, so name nothing rather
    // than risk naming the wrong replacement.
    let normalized = normalize_expression(line);
    if source
        .lines()
        .filter(|other| normalize_expression(other) == normalized)
        .nth(1)
        .is_some()
    {
        return None;
    }
    let id = diff_probe_id(relative, &probe.family, probe.owner.as_ref(), line, 1).0;
    (id != probe.id.0).then_some(id)
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
    crate::analysis::stable_path_text(path)
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

    #[cfg(unix)]
    #[test]
    fn sanitize_path_keeps_invalid_bytes_distinct_from_utf8_text() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let invalid = PathBuf::from(OsString::from_vec(b"pricing_\xff.rs".to_vec()));
        let literal = PathBuf::from("pricing_%FF.rs");
        assert_ne!(sanitize_path(&invalid), sanitize_path(&literal));
    }

    /// #4736: pins the exact id ripr 0.10.0 printed for the boundary_gap
    /// change, so a 0.10 suppression selector can be named and replaced.
    fn read_fixture(path: &Path) -> String {
        std::fs::read_to_string(path).unwrap_or_default()
    }

    #[test]
    fn legacy_whole_line_id_reproduces_the_ripr_0_10_boundary_gap_id() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("ripr-legacy-probe-id-{}", std::process::id()));
        let file = root.join("src").join("lib.rs");
        let written = std::fs::create_dir_all(root.join("src")).and_then(|()| {
            std::fs::write(
                &file,
                "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {\n    if amount >= discount_threshold {\n        amount - 10\n    } else {\n        amount\n    }\n}\n",
            )
        });
        let owner = SymbolId("src/lib.rs::discounted_total".to_string());
        let probe = crate::domain::Probe {
            id: diff_probe_id(
                Path::new("src/lib.rs"),
                &ProbeFamily::Predicate,
                Some(&owner),
                "amount >= discount_threshold",
                1,
            ),
            location: crate::domain::SourceLocation::new(file.clone(), 2, 1),
            owner: Some(owner),
            family: ProbeFamily::Predicate,
            delta: crate::domain::DeltaKind::Control,
            before: None,
            after: Some("amount >= discount_threshold".to_string()),
            expression: "amount >= discount_threshold".to_string(),
            expected_sinks: vec![],
            required_oracles: vec![],
        };
        let legacy = legacy_whole_line_diff_probe_id(&root, &probe, &read_fixture(&file));
        let mut unchanged = probe.clone();
        unchanged.id = diff_probe_id(
            Path::new("src/lib.rs"),
            &ProbeFamily::Predicate,
            unchanged.owner.as_ref(),
            "if amount >= discount_threshold {",
            1,
        );
        let same_as_legacy =
            legacy_whole_line_diff_probe_id(&root, &unchanged, &read_fixture(&file));
        // The same line twice: 0.10 gave one occurrence a `.2` suffix, and
        // which one cannot be recovered from a single finding.
        let repeated = std::fs::write(
            &file,
            "pub fn discounted_total(amount: i32, discount_threshold: i32) -> i32 {\n    if amount >= discount_threshold {\n        amount - 10\n    } else {\n        amount\n    }\n}\npub fn other(amount: i32, discount_threshold: i32) -> bool {\n    if amount >= discount_threshold {\n        return true;\n    }\n    false\n}\n",
        );
        let ambiguous = legacy_whole_line_diff_probe_id(&root, &probe, &read_fixture(&file));
        let _ = std::fs::remove_dir_all(&root);
        repeated.map_err(|e| format!("rewrite fixture failed: {e}"))?;
        assert_eq!(ambiguous, None);
        written.map_err(|e| format!("write fixture failed: {e}"))?;

        assert_eq!(probe.id.0, "probe:src_lib.rs:predicate:c80557eb");
        assert_eq!(
            legacy.as_deref(),
            Some("probe:src_lib.rs:predicate:8390dd91")
        );
        // A finding whose id never changed has no separate legacy id.
        assert_eq!(same_as_legacy, None);
        Ok(())
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
