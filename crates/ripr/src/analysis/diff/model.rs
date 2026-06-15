use std::path::PathBuf;

#[derive(Clone, Debug, Default)]
pub struct ChangedFile {
    pub path: PathBuf,
    pub added_lines: Vec<ChangedLine>,
    pub removed_lines: Vec<ChangedLine>,
}

/// A line that was added or removed in a diff hunk.
///
/// For **added** lines, `line` is the new-file line number and `new_side_line`
/// is always equal to `line`.
///
/// For **removed** lines, `line` is the OLD-file line number (the coordinate
/// recorded by the parser's old-side counter). `new_side_line` is the
/// corresponding position in the NEW file — i.e. the parser's `new_line`
/// counter at the moment the removed line was encountered. When `line ==
/// new_side_line` the two coordinate systems agree (no accumulated net delta
/// above this hunk). When they diverge, using `line` to index into the new
/// file would point at the wrong location.
///
/// Always use `new_side_line` to form a `SourceLocation` that targets the
/// new file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedLine {
    /// Old-side line number for removed lines; new-side line number for added
    /// lines. Use `new_side_line` for any location that must point into the
    /// new (on-disk) file.
    pub line: usize,
    pub text: String,
    /// New-file line number. For added lines this equals `line`. For removed
    /// lines this is the new-side position at the moment the parser saw the
    /// `-` prefix — it may differ from `line` when an earlier hunk shifted the
    /// coordinate systems.
    pub new_side_line: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_types_are_constructible() {
        let changed_file = ChangedFile {
            path: PathBuf::from("src/lib.rs"),
            added_lines: vec![],
            removed_lines: vec![],
        };
        assert_eq!(changed_file.path, PathBuf::from("src/lib.rs"));
        assert!(changed_file.added_lines.is_empty());
        assert!(changed_file.removed_lines.is_empty());

        let changed_line = ChangedLine {
            line: 42,
            text: "example".to_string(),
            new_side_line: 42,
        };
        assert_eq!(changed_line.line, 42);
        assert_eq!(changed_line.new_side_line, 42);
        assert_eq!(changed_line.text, "example");
    }
}
