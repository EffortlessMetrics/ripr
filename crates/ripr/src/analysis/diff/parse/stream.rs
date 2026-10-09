use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::PathBuf;

use super::{
    ChangedFile, DIFF_FILE_LIMIT_ENV, ParsedDiff, is_new_path_marker, parse_old_path_marker,
    parser_state,
};

/// A borrowed complete byte record. The slice includes its original LF/CRLF
/// terminator; semantic text follows exactly the existing str::lines contract.
/// Production currently supplies already-decoded strings, not original Git
/// stdout. This view establishes no immutable subject or occurrence authority.
struct RawDiffRecord<'a> {
    bytes: &'a [u8],
}

struct RawDiffRecords<'a> {
    records: std::slice::SplitInclusive<'a, u8, fn(&u8) -> bool>,
}

impl<'a> RawDiffRecords<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            records: input.split_inclusive(is_record_line_feed as fn(&u8) -> bool),
        }
    }
}

fn is_record_line_feed(byte: &u8) -> bool {
    *byte == b'\n'
}

impl<'a> Iterator for RawDiffRecords<'a> {
    type Item = RawDiffRecord<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.records.next().map(|bytes| RawDiffRecord { bytes })
    }
}

trait ParserLine {
    fn semantic_text(&self) -> std::borrow::Cow<'_, str>;
}

impl ParserLine for RawDiffRecord<'_> {
    fn semantic_text(&self) -> std::borrow::Cow<'_, str> {
        let text = match self.bytes.strip_suffix(b"\n") {
            Some(before_lf) => before_lf.strip_suffix(b"\r").unwrap_or(before_lf),
            None => self.bytes,
        };
        String::from_utf8_lossy(text)
    }
}

#[cfg(test)]
impl ParserLine for &str {
    fn semantic_text(&self) -> std::borrow::Cow<'_, str> {
        std::borrow::Cow::Borrowed(self)
    }
}

pub(super) fn parse_unbounded(input: &str) -> ParsedDiff {
    match parse_lines(RawDiffRecords::new(input.as_bytes()), |_| {
        Ok::<(), Infallible>(())
    }) {
        Ok(parsed) => parsed,
        Err(never) => match never {},
    }
}

pub(super) fn parse_bounded(input: &str, limit: usize) -> Result<ParsedDiff, String> {
    parse_bounded_records(RawDiffRecords::new(input.as_bytes()), limit)
}

#[cfg(test)]
pub(super) fn parse_bounded_lines<'a>(
    lines: impl Iterator<Item = &'a str>,
    limit: usize,
) -> Result<ParsedDiff, String> {
    parse_bounded_records(lines, limit)
}

fn parse_bounded_records<L: ParserLine>(
    lines: impl Iterator<Item = L>,
    limit: usize,
) -> Result<ParsedDiff, String> {
    parse_lines(lines, |count| {
        #[cfg(test)]
        if crate::analysis::source_calibration::active() {
            crate::analysis::cancellation::checkpoint()?;
            crate::analysis::source_calibration::put(
                "parser_admission",
                serde_json::json!({"accepted_path_minimum": count, "limit": limit, "refused": count > limit}),
            );
        }
        if count <= limit {
            return Ok(());
        }
        Err(format!(
            "diff_scope_oversized: at least {count} changed files exceed the \
             {DIFF_FILE_LIMIT_ENV} limit ({limit}); parsing stopped before the \
             remaining file bodies and analysis was not run. Repair route: reduce \
             the diff scope, split the extraction PR, run a narrower diff, or \
             raise the limit via {DIFF_FILE_LIMIT_ENV}=<number>."
        ))
    })
}

/// Both entry points use the same grammar and accepted-path map. The admission
/// check runs at the two registration sites, before reading another input line.
/// The unbounded caller has an infallible policy rather than a fallback that
/// could accidentally turn a refused parse into an empty successful result.
fn parse_lines<L: ParserLine, E>(
    lines: impl Iterator<Item = L>,
    mut admit_file_count: impl FnMut(usize) -> Result<(), E>,
) -> Result<ParsedDiff, E> {
    let mut files: BTreeMap<PathBuf, ChangedFile> = BTreeMap::new();
    let mut state = parser_state::ParserState::default();
    // One-line lookahead preserves plain ---/+++ section boundaries without
    // collecting the whole input. The input &str itself is already in memory;
    // this file-count policy does not bound its bytes or a single large hunk.
    let mut lines = lines.peekable();
    while let Some(line) = lines.next() {
        let semantic = line.semantic_text();
        let raw = semantic.as_ref();
        if state.handle_diff_boundary(raw) {
            continue;
        }

        if state.handle_binary_files_sentinel(raw) {
            state.record_binary_deletion(raw);
            continue;
        }

        state.note_symlink_header(raw);

        if state.handle_submodule_mode(raw) {
            continue;
        }

        if state.handle_submodule_index(raw) {
            continue;
        }

        if state.handle_rename_metadata(raw, &mut files) {
            admit_file_count(files.len())?;
            continue;
        }

        if state.combined_quarantine()
            && parse_old_path_marker(raw)
            && lines
                .peek()
                .is_some_and(|next| is_new_path_marker(&next.semantic_text()))
        {
            // Unprefixed plain markers may follow a combined hunk without a
            // diff --git boundary; quarantined parent columns must stay inert.
            state.close_combined_quarantine();
        }

        if state.in_hunk()
            && state.can_end_at_plain_boundary()
            && parse_old_path_marker(raw)
            && lines
                .peek()
                .is_some_and(|next| is_new_path_marker(&next.semantic_text()))
        {
            state.close_hunk();
        }

        if state.register_path_marker(raw, &mut files) {
            admit_file_count(files.len())?;
            continue;
        }

        if state.handle_hunk_header(raw) {
            continue;
        }

        state.consume_hunk_line(raw, &mut files);
    }

    // End of stream closes a hunk that was still open: lines it promised but
    // never delivered are malformed. #4375: end of stream also closes the
    // final open file section, so a section truncated at EOF counts exactly
    // like one closed by a later boundary.
    state.close_hunk();
    state.close_file_section_accounting();
    Ok(ParsedDiff {
        changed_files: files.into_values().collect(),
        deleted_file_count: state.deleted_file_count(),
        submodule_file_count: state.submodule_file_count(),
        renamed_file_count: state.renamed_file_count(),
        pure_rename_file_count: state.pure_rename_file_count(),
        pure_rename_paths: state.pure_rename_paths(),
        truncated_file_sections: state.truncated_file_sections(),
        raw_line1_bom_paths: state.raw_line1_bom_paths(),
        limitations: state.limitations(),
    })
}

#[cfg(test)]
mod tests;
