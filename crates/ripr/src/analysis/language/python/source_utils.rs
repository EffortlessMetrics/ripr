use rustpython_parser::text_size::TextRange;
use std::path::Path;

/// A Python source text with a line-start index built once per file.
///
/// Fact extraction asks for the line of many AST ranges in the same file.
/// Counting newlines from byte 0 on every lookup made extraction
/// O(nodes x file length) (#4495); this index answers each lookup by binary
/// search instead. It dereferences to the source `str`, so text slicing and
/// `&str` helpers keep working unchanged.
pub(super) struct SourceText<'a> {
    text: &'a str,
    /// Byte offsets of every `\n`, in ascending order.
    newline_offsets: Vec<usize>,
    /// `typer.Typer()` receiver names, scanned once on first use; every
    /// owner in the file asks for them.
    cli_receiver_names: std::cell::OnceCell<Vec<String>>,
}

impl<'a> SourceText<'a> {
    pub(super) fn new(text: &'a str) -> Self {
        let newline_offsets = text
            .bytes()
            .enumerate()
            .filter_map(|(idx, byte)| (byte == b'\n').then_some(idx))
            .collect();
        Self {
            text,
            newline_offsets,
            cli_receiver_names: std::cell::OnceCell::new(),
        }
    }

    /// 1-indexed line for a 0-indexed byte offset.
    ///
    /// The line is one plus the number of `\n` bytes strictly before
    /// `offset`. A `\r` is not a line break, so CRLF counts once. An offset
    /// past the end of the text reports the last line, and an offset inside
    /// a multibyte character counts only newlines before it. `\n` is a
    /// single-byte UTF-8 character that never appears inside a multibyte
    /// sequence, so counting `\n` bytes equals counting `\n` characters.
    pub(super) fn line_for_offset(&self, offset: usize) -> usize {
        1 + self
            .newline_offsets
            .partition_point(|&newline| newline < offset)
    }

    /// The file's `NAME = typer.Typer(...)` receivers, computed by `scan` on
    /// the first call and reused after.
    pub(super) fn cli_receiver_names(&self, scan: impl FnOnce(&str) -> Vec<String>) -> &[String] {
        self.cli_receiver_names.get_or_init(|| scan(self.text))
    }
}

impl std::ops::Deref for SourceText<'_> {
    type Target = str;

    fn deref(&self) -> &str {
        self.text
    }
}

pub(super) fn line_for_range_start(source: &SourceText<'_>, range: TextRange) -> usize {
    source.line_for_offset(usize::from(range.start()))
}

pub(super) fn line_for_range_end(source: &SourceText<'_>, range: TextRange) -> usize {
    source.line_for_offset(usize::from(range.end()))
}

pub(super) fn text_for_range(source: &str, range: TextRange) -> String {
    let start = usize::from(range.start()).min(source.len());
    let end = usize::from(range.end()).min(source.len());
    source.get(start..end).unwrap_or_default().to_string()
}

pub(super) fn normalized_path(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    normalized
}

pub(super) fn is_test_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if file_name.starts_with("test_") || file_name.ends_with("_test.py") {
        return true;
    }
    path.components().any(|component| {
        let text = component.as_os_str().to_string_lossy();
        text == "tests" || text == "test"
    })
}
