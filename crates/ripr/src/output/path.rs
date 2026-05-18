use std::path::Path;

/// Render a path with stable slash separators for JSON and Markdown output.
pub(crate) fn display_path(path: &Path) -> String {
    display_path_text(&path.to_string_lossy())
}

/// Render path-like text with stable slash separators for JSON and Markdown output.
pub(crate) fn display_path_text(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{display_path, display_path_text};

    #[test]
    fn display_path_normalizes_backslashes() {
        assert_eq!(display_path(Path::new("a\\b\\c")), "a/b/c");
    }

    #[test]
    fn display_path_text_normalizes_backslashes() {
        assert_eq!(display_path_text("a\\b\\c"), "a/b/c");
    }

    #[test]
    fn display_path_preserves_forward_slashes() {
        assert_eq!(display_path(Path::new("a/b/c")), "a/b/c");
    }
}
