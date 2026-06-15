use crate::domain::Finding;

pub(in crate::app) fn select_finding<'a>(
    findings: &'a [Finding],
    selector: &str,
) -> Option<&'a Finding> {
    findings
        .iter()
        .find(|finding| finding.id == selector || selector_matches_location(selector, finding))
}

pub(in crate::app) fn selector_matches_location(selector: &str, finding: &Finding) -> bool {
    let file = finding.probe.location.file.to_string_lossy();
    selector_matches_file_line(file.as_ref(), finding.probe.location.line, selector)
}

/// Resolve a `file:line` locator against a stored finding location.
///
/// Match the natural path a user sees, not the exact internal string. The
/// stored path carries a `.\`/`./` prefix and platform separators (e.g.
/// `.\crates/ripr/src/lib.rs`); normalize both sides, then accept an exact
/// match or a path-suffix match in either direction so that `src/lib.rs:8` and
/// `crates/ripr/src/lib.rs:8` both resolve.
fn selector_matches_file_line(stored_file: &str, stored_line: usize, selector: &str) -> bool {
    let Some((selector_file, selector_line)) = selector.rsplit_once(':') else {
        return false;
    };
    if selector_line != stored_line.to_string() {
        return false;
    }
    let stored = normalize_locator_path(stored_file);
    let wanted = normalize_locator_path(selector_file);
    if wanted.is_empty() || stored.is_empty() {
        return false;
    }
    stored == wanted
        || stored.ends_with(&format!("/{wanted}"))
        || wanted.ends_with(&format!("/{stored}"))
}

/// Normalize a path for locator matching: convert `\` to `/`, drop a leading
/// `./`, and trim a trailing `/`. Lets the path a user copies from `check`
/// output (or types naturally) match the stored form regardless of platform
/// separators or the leading-current-directory prefix.
fn normalize_locator_path(path: &str) -> String {
    let forward = path.replace('\\', "/");
    let trimmed = forward.strip_prefix("./").unwrap_or(forward.as_str());
    trimmed.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::{normalize_locator_path, selector_matches_file_line};

    // Stored form mirrors real `check` output: a `.\` prefix and forward slashes.
    const STORED: &str = ".\\crates/ripr/examples/sample/src/lib.rs";

    #[test]
    fn exact_internal_path_still_matches() {
        // Backward compatible: the verbatim stored string keeps working.
        assert!(selector_matches_file_line(
            STORED,
            8,
            ".\\crates/ripr/examples/sample/src/lib.rs:8"
        ));
    }

    #[test]
    fn natural_full_path_matches() {
        assert!(selector_matches_file_line(
            STORED,
            8,
            "crates/ripr/examples/sample/src/lib.rs:8"
        ));
    }

    #[test]
    fn natural_suffix_matches() {
        assert!(selector_matches_file_line(STORED, 8, "src/lib.rs:8"));
    }

    #[test]
    fn pasted_windows_separators_match() {
        assert!(selector_matches_file_line(
            STORED,
            8,
            "crates\\ripr\\examples\\sample\\src\\lib.rs:8"
        ));
    }

    #[test]
    fn line_must_match() {
        assert!(!selector_matches_file_line(STORED, 8, "src/lib.rs:9"));
        assert!(selector_matches_file_line(STORED, 8, "src/lib.rs:8"));
    }

    #[test]
    fn unrelated_file_does_not_match() {
        assert!(!selector_matches_file_line(STORED, 8, "src/other.rs:8"));
    }

    #[test]
    fn malformed_selector_does_not_match() {
        // No line component.
        assert!(!selector_matches_file_line(STORED, 8, "src/lib.rs"));
        // Empty file component.
        assert!(!selector_matches_file_line(STORED, 8, ":8"));
    }

    #[test]
    fn normalize_strips_prefix_and_separators() {
        assert_eq!(normalize_locator_path(".\\a/b/c.rs"), "a/b/c.rs");
        assert_eq!(normalize_locator_path("./a/b/c.rs"), "a/b/c.rs");
        assert_eq!(normalize_locator_path("a\\b\\c.rs"), "a/b/c.rs");
        assert_eq!(normalize_locator_path("a/b/c.rs/"), "a/b/c.rs");
    }
}
