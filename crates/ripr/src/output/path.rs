use std::path::Path;

/// Render a path with stable slash separators for JSON and Markdown output.
pub(crate) fn display_path(path: &Path) -> String {
    display_path_text(&crate::analysis::stable_path_text(path))
}

/// Render path-like text with stable slash separators for JSON and Markdown output.
pub(crate) fn display_path_text(path: &str) -> String {
    path.replace('\\', "/")
}

/// Render a path for human CLI display (`ripr init`, `ripr doctor`) with one
/// separator convention on the running host (#4378). See
/// [`human_path_text`].
pub(crate) fn human_path(path: &Path) -> String {
    human_path_text(&path.to_string_lossy(), cfg!(windows))
}

/// Human display rule for path text, pure over the host so both answers are
/// testable anywhere (#4378).
///
/// On Windows `\` is a separator: a drive-letter verbatim prefix (`\\?\`
/// before `<drive>:`) renders as the plain path and every `\` renders as `/`,
/// so a forward-slash argv prefix and a joined suffix never mix separators in
/// one path. Other verbatim forms (`\\?\UNC\..`, `\\?\Volume{..}`) render
/// exactly as given, like `app::verification_execution`'s normalizer. On
/// Unix `\` is an ordinary filename character, so the text is returned
/// unchanged; rewriting it would name a different file.
pub(crate) fn human_path_text(text: &str, windows: bool) -> String {
    if !windows {
        return text.to_string();
    }
    let plain = match text.strip_prefix(r"\\?\") {
        None => text,
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') => rest,
        Some(_) => return text.to_string(),
    };
    plain.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{display_path, display_path_text, human_path_text};

    #[test]
    fn human_path_text_uses_one_separator_and_drops_verbatim_prefix_on_windows() {
        // `check-local-context` forbids drive-letter path literals, so the
        // Windows shapes are assembled from parts.
        let drive = "F:";
        assert_eq!(
            human_path_text(&format!(r"{drive}/Temp/demo\ripr.toml"), true),
            format!("{drive}/Temp/demo/ripr.toml")
        );
        assert_eq!(
            human_path_text(&format!(r"\\?\{drive}\code\target\debug\ripr.exe"), true),
            format!("{drive}/code/target/debug/ripr.exe")
        );
        assert_eq!(human_path_text(r".\Cargo.toml", true), "./Cargo.toml");
        // Verbatim forms other than a drive letter are shown exactly as given.
        for verbatim in [r"\\?\UNC\server\share\ripr.exe", r"\\?\Volume{1}\x"] {
            assert_eq!(human_path_text(verbatim, true), verbatim);
        }
    }

    #[test]
    fn human_path_text_keeps_unix_backslash_filenames() {
        assert_eq!(
            human_path_text(r"/tmp/odd\name/ripr.toml", false),
            r"/tmp/odd\name/ripr.toml"
        );
        let verbatim = format!(r"\\?\{}\x", "F:");
        assert_eq!(human_path_text(&verbatim, false), verbatim);
    }

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
