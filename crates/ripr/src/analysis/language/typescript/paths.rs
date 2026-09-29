//! Path utilities for the TypeScript preview adapter.

use super::*;

/// A TypeScript/JavaScript source text with a line index built once per
/// source.
///
/// Owner, oracle and test extraction ask for the line of many oxc spans in
/// the same file. Counting newlines from byte 0 on every lookup made
/// extraction O(nodes x file length); this index answers each lookup by
/// binary search instead. It dereferences to the source `str`, so slicing
/// and `&str` helpers keep working unchanged.
pub(crate) struct SourceText<'a> {
    text: &'a str,
    /// Byte offsets of every `\n`, in ascending order.
    newline_offsets: Vec<usize>,
}

impl<'a> SourceText<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        let newline_offsets = text
            .bytes()
            .enumerate()
            .filter_map(|(idx, byte)| (byte == b'\n').then_some(idx))
            .collect();
        Self {
            text,
            newline_offsets,
        }
    }

    /// 1-indexed line for a 0-indexed byte offset (an oxc `u32` span bound
    /// widened to `usize`).
    ///
    /// The line is one plus the number of `\n` bytes strictly before
    /// `offset`. A `\r` is not a line break, so CRLF counts once and a lone
    /// `\r` never starts a line. An offset past the end of the text reports
    /// the last line, and an offset inside a multibyte character counts only
    /// the newlines before it. `\n` is a single-byte UTF-8 character that
    /// never appears inside a multibyte sequence, so counting `\n` bytes
    /// equals counting `\n` characters.
    pub(crate) fn line_for_offset(&self, offset: usize) -> usize {
        1 + self
            .newline_offsets
            .partition_point(|&newline| newline < offset)
    }
}

impl std::ops::Deref for SourceText<'_> {
    type Target = str;

    fn deref(&self) -> &str {
        self.text
    }
}

/// The pre-index per-call scan: 1-indexed line for a 0-indexed byte offset,
/// counting newlines from byte 0. Kept only as the test oracle that
/// [`SourceText::line_for_offset`] must match exactly.
#[cfg(test)]
pub(crate) fn line_for_offset(source: &str, offset: usize) -> usize {
    let mut line: usize = 1;
    for (idx, ch) in source.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
        }
    }
    line
}

pub(crate) fn normalized_path(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    normalized
}

pub(crate) fn output_language_for(path: &Path) -> DomainLanguageId {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("js" | "jsx" | "mjs" | "cjs") => DomainLanguageId::JavaScript,
        _ => DomainLanguageId::TypeScript,
    }
}
