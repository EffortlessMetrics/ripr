use std::collections::BTreeMap;
use std::convert::Infallible;
use std::path::{Path, PathBuf};

use super::{
    BodyDisposition, BodyOutcome, ChangedFile, DIFF_FILE_LIMIT_ENV, HunkHeader, HunkOutcome,
    ParsedDiff, PathMarkerOutcome, is_new_path_marker, parse_old_path_marker, parser_state,
};

/// A borrowed complete byte record. The slice includes its original LF/CRLF
/// terminator; semantic text follows exactly the existing str::lines contract.
/// Ordinary callers supply decoded strings; the observed entry supplies
/// original Git stdout. Neither entry authenticates an immutable subject.
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
    match parse_lines(
        RawDiffRecords::new(input.as_bytes()),
        |_| Ok::<(), Infallible>(()),
        NoopObserver,
    ) {
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
    parse_lines(lines, admit_count(limit), NoopObserver)
}

fn admit_count(limit: usize) -> impl FnMut(usize) -> Result<(), String> {
    move |count| {
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
    }
}

/// Original-byte data only. The producer owns subject authentication,
/// retention budgets, inventory reconciliation and completion authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RawRecord<'a> {
    pub(crate) ordinal: usize,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) bytes: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RawRecordKind {
    GitBoundary,
    Binary,
    SubmoduleMode,
    SubmoduleIndex,
    Rename,
    PathMarker(PathMarkerOutcome),
    HunkHeader,
    CombinedHunk,
    MalformedHunk,
    Body(BodyDisposition),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RawChangeSide {
    Added,
    Removed,
}

#[derive(Debug)]
pub(crate) struct RawReduction<'a> {
    pub(crate) record: RawRecord<'a>,
    /// Only actual Git boundary winners establish section identity.
    /// Plain-marker transitions are deliberately unanchored.
    pub(crate) section: Option<usize>,
    pub(crate) hunk: Option<usize>,
    pub(crate) kind: RawRecordKind,
    pub(crate) declared_ranges: Option<((usize, usize), (usize, usize))>,
    pub(crate) remaining_before: Option<(usize, usize)>,
    pub(crate) remaining_after: Option<(usize, usize)>,
    pub(crate) consumed: Option<(usize, usize)>,
    /// Declared range plus consumed count, independent of advisory cursors.
    /// None after malformed/excess accounting or coordinate overflow.
    pub(crate) coordinates: Option<(usize, usize)>,
    pub(crate) raw_path_token: Option<&'a [u8]>,
    /// None means no token or unsupported/malformed token, never clean success.
    pub(crate) decoded_path_bytes: Option<Vec<u8>>,
    pub(crate) native_path: Option<&'a Path>,
    /// Exact side-vector insertion slot; several occurrences may share a path.
    pub(crate) projection: Option<(RawChangeSide, usize)>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RawEnd {
    pub(crate) bytes: usize,
    pub(crate) records: usize,
    pub(crate) sections: usize,
    pub(crate) hunks: usize,
}

/// Callbacks are synchronous and fallible. A refused callback/admission
/// yields Err with no end callback. End reports consumption, not eligibility.
pub(crate) trait RawDiffObserver {
    fn record(&mut self, record: RawRecord<'_>) -> Result<(), String>;
    fn reduction(&mut self, reduction: RawReduction<'_>) -> Result<(), String>;
    fn finish(&mut self, end: RawEnd, parsed: &ParsedDiff) -> Result<(), String>;
}

#[cfg_attr(not(test), expect(dead_code, reason = "Inactive producer-owned complete route consumes this crate-private seam"))]
pub(crate) fn parse_bytes_bounded(
    input: &[u8],
    limit: usize,
    observer: &mut impl RawDiffObserver,
) -> Result<ParsedDiff, String> {
    parse_lines(
        RawDiffRecords::new(input),
        admit_count(limit),
        OriginalByteObserver {
            observer,
            expected_bytes: input.len(),
            end: RawEnd { bytes: 0, records: 0, sections: 0, hunks: 0 },
            pending: None,
            section: None,
            hunk: None,
            header: None,
            marker_opened: false,
        },
    )
}

#[derive(Clone, Copy)]
enum Reduction {
    GitBoundary,
    Binary,
    SubmoduleMode,
    SubmoduleIndex,
    Rename,
    PathMarker(PathMarkerOutcome),
    Hunk(HunkOutcome),
    Body(BodyOutcome),
}

trait ParserObserver<L, E> {
    fn begin(&mut self, line: &L) -> Result<(), E>;
    fn reduce(
        &mut self,
        line: &L,
        reduction: Reduction,
        state: &parser_state::ParserState,
    ) -> Result<(), E>;
    fn plain_boundary(&mut self);
    fn finish(&mut self, parsed: &ParsedDiff) -> Result<(), E>;
}

/// Monomorphized ordinary route: no ledger, native path clone or traversal.
struct NoopObserver;

impl<L, E> ParserObserver<L, E> for NoopObserver {
    fn begin(&mut self, _line: &L) -> Result<(), E> { Ok(()) }
    fn reduce(
        &mut self,
        _line: &L,
        _reduction: Reduction,
        _state: &parser_state::ParserState,
    ) -> Result<(), E> { Ok(()) }
    fn plain_boundary(&mut self) {}
    fn finish(&mut self, _parsed: &ParsedDiff) -> Result<(), E> { Ok(()) }
}

struct OriginalByteObserver<'a, 's, O> {
    observer: &'s mut O,
    expected_bytes: usize,
    end: RawEnd,
    pending: Option<RawRecord<'a>>,
    section: Option<usize>,
    hunk: Option<usize>,
    header: Option<HunkHeader>,
    marker_opened: bool,
}

impl<'a, O: RawDiffObserver> ParserObserver<RawDiffRecord<'a>, String>
    for OriginalByteObserver<'a, '_, O>
{
    fn begin(&mut self, line: &RawDiffRecord<'a>) -> Result<(), String> {
        let start = self.end.bytes;
        let end = start.checked_add(line.bytes.len())
            .ok_or_else(|| "raw diff byte offset overflow".to_string())?;
        let next = self.end.records.checked_add(1)
            .ok_or_else(|| "raw diff record ordinal overflow".to_string())?;
        if end > self.expected_bytes || self.pending.is_some() {
            return Err("raw diff record interval mismatch".to_string());
        }
        let record = RawRecord { ordinal: self.end.records, start, end, bytes: line.bytes };
        // This callback precedes semantic decoding and every parser handler.
        self.observer.record(record)?;
        self.end.bytes = end;
        self.end.records = next;
        self.pending = Some(record);
        Ok(())
    }

    fn reduce(
        &mut self,
        _line: &RawDiffRecord<'a>,
        reduction: Reduction,
        state: &parser_state::ParserState,
    ) -> Result<(), String> {
        let record = self.pending.take()
            .ok_or_else(|| "raw diff reduction without consumed record".to_string())?;
        let mut body = None;
        let kind = match reduction {
            Reduction::GitBoundary => {
                self.section = Some(self.end.sections);
                self.end.sections = self.end.sections.checked_add(1)
                    .ok_or_else(|| "raw diff section ordinal overflow".to_string())?;
                self.hunk = None;
                self.header = None;
                self.marker_opened = false;
                RawRecordKind::GitBoundary
            }
            Reduction::Binary => {
                self.hunk = None;
                self.header = None;
                RawRecordKind::Binary
            }
            Reduction::SubmoduleMode => RawRecordKind::SubmoduleMode,
            Reduction::SubmoduleIndex => RawRecordKind::SubmoduleIndex,
            Reduction::Rename => RawRecordKind::Rename,
            Reduction::PathMarker(outcome) => {
                if outcome == PathMarkerOutcome::Old && self.marker_opened {
                    self.plain_boundary();
                }
                if matches!(outcome, PathMarkerOutcome::New { opened: true }) {
                    self.marker_opened = true;
                }
                RawRecordKind::PathMarker(outcome)
            }
            Reduction::Hunk(outcome) => {
                self.hunk = Some(self.end.hunks);
                self.end.hunks = self.end.hunks.checked_add(1)
                    .ok_or_else(|| "raw diff hunk ordinal overflow".to_string())?;
                match outcome {
                    HunkOutcome::Ordinary(header) => {
                        self.header = Some(header);
                        RawRecordKind::HunkHeader
                    }
                    HunkOutcome::Combined => {
                        self.header = None;
                        RawRecordKind::CombinedHunk
                    }
                    HunkOutcome::Malformed => {
                        self.header = None;
                        RawRecordKind::MalformedHunk
                    }
                }
            }
            Reduction::Body(outcome) => {
                body = Some(outcome);
                RawRecordKind::Body(outcome.disposition)
            }
        };
        let accounting = body.map(|outcome| outcome.accounting).unwrap_or_default();
        let coordinates = match (self.header, accounting.before, accounting.after, accounting.consumed) {
            (Some(header), Some((old, new)), Some(_), Some(_)) => {
                header.old_count.checked_sub(old).zip(header.new_count.checked_sub(new))
                    .and_then(|(old_used, new_used)| {
                        header.old_start.checked_add(old_used)
                            .zip(header.new_start.checked_add(new_used))
                    })
            }
            _ => None,
        };
        let projection = match body.map(|outcome| outcome.disposition) {
            Some(BodyDisposition::Added(index)) => Some((RawChangeSide::Added, index)),
            Some(BodyDisposition::Removed(index)) => Some((RawChangeSide::Removed, index)),
            _ => None,
        };
        // Exact token spans are available only where that handler won.
        // A Git header remains an opaque original record; no second lexer.
        let raw = record_semantic_bytes(record.bytes);
        let raw_path_token = match kind {
            RawRecordKind::PathMarker(_) =>
                raw.strip_prefix(b"--- ").or_else(|| raw.strip_prefix(b"+++ ")),
            RawRecordKind::Rename =>
                raw.strip_prefix(b"rename from ").or_else(|| raw.strip_prefix(b"rename to ")),
            _ => None,
        };
        let decoded_path_bytes = raw_path_token
            .and_then(crate::analysis::diff::path::parse_diff_path_token_bytes);
        self.observer.reduction(RawReduction {
            record, section: self.section, hunk: self.hunk, kind,
            declared_ranges: self.header.map(|h| ((h.old_start, h.old_count), (h.new_start, h.new_count))),
            remaining_before: accounting.before,
            remaining_after: accounting.after,
            consumed: accounting.consumed,
            coordinates, raw_path_token, decoded_path_bytes,
            native_path: state.observed_path(), projection,
        })
    }

    fn plain_boundary(&mut self) {
        self.section = None;
        self.hunk = None;
        self.header = None;
        self.marker_opened = false;
    }

    fn finish(&mut self, parsed: &ParsedDiff) -> Result<(), String> {
        if self.pending.is_some() || self.end.bytes != self.expected_bytes {
            return Err("raw diff end before exhaustive consumption".to_string());
        }
        self.observer.finish(self.end, parsed)
    }
}

fn record_semantic_bytes(bytes: &[u8]) -> &[u8] {
    match bytes.strip_suffix(b"\n") {
        Some(before_lf) => before_lf.strip_suffix(b"\r").unwrap_or(before_lf),
        None => bytes,
    }
}

/// Both entry points use the same grammar and accepted-path map. The admission
/// check runs at the two registration sites, before reading another input line.
/// The unbounded caller has an infallible policy rather than a fallback that
/// could accidentally turn a refused parse into an empty successful result.
fn parse_lines<L: ParserLine, E>(
    lines: impl Iterator<Item = L>,
    mut admit_file_count: impl FnMut(usize) -> Result<(), E>,
    mut observer: impl ParserObserver<L, E>,
) -> Result<ParsedDiff, E> {
    let mut files: BTreeMap<PathBuf, ChangedFile> = BTreeMap::new();
    let mut state = parser_state::ParserState::default();
    // One-line lookahead preserves plain ---/+++ section boundaries without
    // collecting the whole input. The input &str itself is already in memory;
    // this file-count policy does not bound its bytes or a single large hunk.
    let mut lines = lines.peekable();
    while let Some(line) = lines.next() {
        // Lookahead never calls begin: only consumed records enter the ledger.
        observer.begin(&line)?;
        let semantic = line.semantic_text();
        let raw = semantic.as_ref();
        if state.handle_diff_boundary(raw) {
            observer.reduce(&line, Reduction::GitBoundary, &state)?;
            continue;
        }

        if state.handle_binary_files_sentinel(raw) {
            state.record_binary_deletion(raw);
            observer.reduce(&line, Reduction::Binary, &state)?;
            continue;
        }

        state.note_symlink_header(raw);

        if state.handle_submodule_mode(raw) {
            observer.reduce(&line, Reduction::SubmoduleMode, &state)?;
            continue;
        }

        if state.handle_submodule_index(raw) {
            observer.reduce(&line, Reduction::SubmoduleIndex, &state)?;
            continue;
        }

        if state.handle_rename_metadata(raw, &mut files) {
            admit_file_count(files.len())?;
            observer.reduce(&line, Reduction::Rename, &state)?;
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
            observer.plain_boundary();
        }

        if state.in_hunk()
            && state.can_end_at_plain_boundary()
            && parse_old_path_marker(raw)
            && lines
                .peek()
                .is_some_and(|next| is_new_path_marker(&next.semantic_text()))
        {
            state.close_hunk();
            observer.plain_boundary();
        }

        if let Some(outcome) = state.register_path_marker(raw, &mut files) {
            admit_file_count(files.len())?;
            observer.reduce(&line, Reduction::PathMarker(outcome), &state)?;
            continue;
        }

        if let Some(outcome) = state.handle_hunk_header(raw) {
            observer.reduce(&line, Reduction::Hunk(outcome), &state)?;
            continue;
        }

        let outcome = state.consume_hunk_line(raw, &mut files);
        observer.reduce(&line, Reduction::Body(outcome), &state)?;
    }

    // End of stream closes a hunk that was still open: lines it promised but
    // never delivered are malformed. #4375: end of stream also closes the
    // final open file section, so a section truncated at EOF counts exactly
    // like one closed by a later boundary.
    state.close_hunk();
    state.close_file_section_accounting();
    let parsed = ParsedDiff {
        changed_files: files.into_values().collect(),
        deleted_file_count: state.deleted_file_count(),
        submodule_file_count: state.submodule_file_count(),
        renamed_file_count: state.renamed_file_count(),
        pure_rename_file_count: state.pure_rename_file_count(),
        pure_rename_paths: state.pure_rename_paths(),
        truncated_file_sections: state.truncated_file_sections(),
        raw_line1_bom_paths: state.raw_line1_bom_paths(),
        limitations: state.limitations(),
    };
    observer.finish(&parsed)?;
    Ok(parsed)
}

#[cfg(test)]
mod tests;
