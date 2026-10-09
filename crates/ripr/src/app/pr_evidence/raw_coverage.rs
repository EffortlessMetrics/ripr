//! Original-byte parser coverage data for the private whole-subject route.
//!
//! This ledger records consumption and semantic projection, not eligibility,
//! analysis success, resource admission, or execution authority. Its verifier
//! replays the existing parser; it never runs a language analyzer. The caller
//! owns aggregate admission for raw bytes, parser state, ledger storage and
//! path counters, and must supply bounds already admitted by that profile.
//! The existing parser constructs supplied decoded/native paths before its
//! reduction callback. The path budget here covers our retained counter clones,
//! not all parser allocations; the parent's finite worker profile owns those.

use crate::analysis::diff::parse::stream::{
    RawChangeSide, RawDiffObserver, RawEnd, RawRecord, RawRecordKind, RawReduction,
    parse_bytes_bounded,
};
use crate::analysis::diff::parse::{BodyDisposition, ParsedDiff, PathMarkerOutcome};
use crate::analysis::diff::{ChangedFile, ChangedLine};
use serde::Serialize;
use serde::ser::{SerializeSeq, SerializeStruct, Serializer};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const LEDGER_SCHEMA: &str = "ripr.raw-parser-coverage.v1";

/// Caller-admitted finite bounds. There are deliberately no defaults or
/// environment-derived overrides here. Zero file/record/path admission is
/// useful for an empty or metadata-only subject and retains parser semantics.
#[derive(Clone, Copy, Debug)]
pub(super) struct RawCoverageLimits {
    pub(super) file_limit: usize,
    pub(super) max_raw_bytes: usize,
    pub(super) max_records: usize,
    pub(super) max_ledger_bytes: usize,
    pub(super) max_retained_path_bytes: usize,
    pub(super) max_projection_bytes: usize,
}

/// Derived data only. None of these fields authorizes complete execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct RawCoverageSummary {
    pub(super) raw_sha256: String,
    pub(super) ledger_sha256: String,
    pub(super) projection_sha256: String,
    pub(super) raw_bytes: usize,
    pub(super) ledger_bytes: usize,
    pub(super) records: usize,
    pub(super) sections: usize,
    pub(super) hunks: usize,
    pub(super) changed_files: usize,
    pub(super) added_lines: usize,
    pub(super) removed_lines: usize,
}

#[derive(Debug)]
pub(super) struct RawCoverage {
    ledger: Vec<u8>,
    summary: RawCoverageSummary,
}

impl RawCoverage {
    pub(super) fn ledger_bytes(&self) -> &[u8] {
        &self.ledger
    }

    pub(super) fn summary(&self) -> &RawCoverageSummary {
        &self.summary
    }
}

/// Parse the admitted original bytes once, retaining their canonical ledger.
/// Parser limitations and advisory changed lines are returned unchanged.
pub(super) fn build_raw_coverage(
    raw: &[u8],
    limits: RawCoverageLimits,
) -> Result<(ParsedDiff, RawCoverage), String> {
    let mut observer = LedgerObserver::new(raw, limits, LedgerSink::Build(Vec::new()))?;
    let parsed = parse_bytes_bounded(raw, limits.file_limit, &mut observer)?;
    let (sink, summary) = observer.into_data()?;
    let LedgerSink::Build(ledger) = sink else {
        return Err("raw coverage build sink mismatch".to_string());
    };
    Ok((parsed, RawCoverage { ledger, summary }))
}

/// Replay the same parser into a bounded comparison sink. No second ledger
/// vector, saved-data deserialization, analyzer run or execution token exists.
/// Missing, changed, duplicated, overlapping or trailing saved facts refuse.
pub(super) fn verify_raw_coverage(
    raw: &[u8],
    saved_ledger: &[u8],
    limits: RawCoverageLimits,
) -> Result<(ParsedDiff, RawCoverageSummary), String> {
    if saved_ledger.len() > limits.max_ledger_bytes {
        return Err("raw coverage saved ledger exceeds its admitted byte bound".to_string());
    }
    let mut observer = LedgerObserver::new(
        raw,
        limits,
        LedgerSink::Compare {
            saved: saved_ledger,
            cursor: 0,
        },
    )?;
    let parsed = parse_bytes_bounded(raw, limits.file_limit, &mut observer)?;
    let (_, summary) = observer.into_data()?;
    Ok((parsed, summary))
}

/// Canonical, bounded, streaming commitment to every ParsedDiff field,
/// including both coordinates, full text, native paths and typed limitations.
/// This is reusable data reconciliation; it confers no execution authority.
pub(super) fn semantic_projection_digest(
    parsed: &ParsedDiff,
    max_projection_bytes: usize,
) -> Result<String, String> {
    if max_projection_bytes == 0 {
        return Err("raw coverage projection byte bound must be positive".to_string());
    }
    let mut writer = ProjectionHasher {
        limit: max_projection_bytes,
        bytes: 0,
        digest: Sha256::new(),
    };
    serde_json::to_writer(&mut writer, &Projection(parsed))
        .map_err(|error| format!("raw coverage projection write failed: {error}"))?;
    Ok(format!("sha256:{:x}", writer.digest.finalize()))
}

enum LedgerSink<'a> {
    Build(Vec<u8>),
    Compare { saved: &'a [u8], cursor: usize },
}

struct LedgerWriter<'a> {
    sink: LedgerSink<'a>,
    limit: usize,
    bytes: usize,
    digest: Sha256,
}

impl Write for LedgerWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let end = self
            .bytes
            .checked_add(buffer.len())
            .filter(|end| *end <= self.limit)
            .ok_or_else(|| io::Error::other("raw coverage ledger byte bound exceeded"))?;
        match &mut self.sink {
            LedgerSink::Build(output) => {
                if end > output.capacity() {
                    // The allocation request is bounded before growth. Geometric
                    // capacity avoids repeated copying for tiny serializer writes.
                    let capacity = end.max(output.capacity().saturating_mul(2).min(self.limit));
                    output
                        .try_reserve_exact(capacity - output.len())
                        .map_err(io::Error::other)?;
                }
                output.extend_from_slice(buffer);
            }
            LedgerSink::Compare { saved, cursor } => {
                if saved.get(*cursor..end) != Some(buffer) {
                    return Err(io::Error::other(format!(
                        "raw coverage canonical ledger differs at byte {}",
                        *cursor
                    )));
                }
                *cursor = end;
            }
        }
        self.digest.update(buffer);
        self.bytes = end;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct ProjectionHasher {
    limit: usize,
    bytes: usize,
    digest: Sha256,
}

impl Write for ProjectionHasher {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        let end = self
            .bytes
            .checked_add(buffer.len())
            .filter(|end| *end <= self.limit)
            .ok_or_else(|| io::Error::other("raw coverage projection byte bound exceeded"))?;
        self.digest.update(buffer);
        self.bytes = end;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Serialize)]
struct Header<'a> {
    tag: &'static str,
    schema: &'static str,
    native_representation: &'static str,
    raw_bytes: usize,
    raw_sha256: &'a str,
    file_limit: usize,
}

struct PendingRecord {
    ordinal: usize,
    start: usize,
    end: usize,
    sha256: String,
}

#[derive(Default)]
struct SideCounts {
    added: usize,
    removed: usize,
}

struct CoverageFacts {
    end: RawEnd,
    projection_sha256: String,
    changed_files: usize,
    added_lines: usize,
    removed_lines: usize,
}

struct LedgerObserver<'raw, 'saved> {
    raw: &'raw [u8],
    limits: RawCoverageLimits,
    raw_sha256: String,
    expected_records: usize,
    writer: LedgerWriter<'saved>,
    pending: Option<PendingRecord>,
    records: usize,
    consumed_bytes: usize,
    sections: usize,
    hunks: usize,
    slots: BTreeMap<PathBuf, SideCounts>,
    retained_path_bytes: usize,
    facts: Option<CoverageFacts>,
}

impl<'raw, 'saved> LedgerObserver<'raw, 'saved> {
    fn new(
        raw: &'raw [u8],
        limits: RawCoverageLimits,
        sink: LedgerSink<'saved>,
    ) -> Result<Self, String> {
        if limits.max_raw_bytes == 0 || limits.max_ledger_bytes == 0 {
            return Err("raw coverage raw and ledger byte bounds must be positive".to_string());
        }
        if limits.max_projection_bytes == 0 {
            return Err("raw coverage projection byte bound must be positive".to_string());
        }
        // Admission precedes hashing, parser construction and semantic decoding.
        if raw.len() > limits.max_raw_bytes {
            return Err("raw coverage input exceeds its admitted byte bound".to_string());
        }
        // Plain-marker lookahead may decode the next record before its callback.
        // Byte framing is preflighted here so the record guard precedes all
        // decoding; no semantic lexer or path decoder is introduced.
        let mut expected_records = 0usize;
        for _ in raw.split_inclusive(|byte| *byte == b'\n') {
            expected_records = expected_records
                .checked_add(1)
                .filter(|count| *count <= limits.max_records)
                .ok_or_else(|| "raw coverage record count bound exceeded".to_string())?;
        }
        let raw_sha256 = format!("sha256:{:x}", Sha256::digest(raw));
        let mut writer = LedgerWriter {
            sink,
            limit: limits.max_ledger_bytes,
            bytes: 0,
            digest: Sha256::new(),
        };
        write_entry(
            &mut writer,
            &Header {
                tag: "header",
                schema: LEDGER_SCHEMA,
                native_representation: native_representation(),
                raw_bytes: raw.len(),
                raw_sha256: &raw_sha256,
                file_limit: limits.file_limit,
            },
        )?;
        Ok(Self {
            raw,
            limits,
            raw_sha256,
            expected_records,
            writer,
            pending: None,
            records: 0,
            consumed_bytes: 0,
            sections: 0,
            hunks: 0,
            slots: BTreeMap::new(),
            retained_path_bytes: 0,
            facts: None,
        })
    }

    fn checked_record(&self, record: RawRecord<'_>) -> Result<&'raw [u8], String> {
        if record.start >= record.end {
            return Err("raw coverage record span is empty or reversed".to_string());
        }
        let bytes = self
            .raw
            .get(record.start..record.end)
            .ok_or_else(|| "raw coverage record span lies outside the original input".to_string())?;
        if !std::ptr::eq(bytes.as_ptr(), record.bytes.as_ptr()) || bytes != record.bytes {
            return Err("raw coverage record is not the helper-owned original subslice".to_string());
        }
        Ok(bytes)
    }

    fn note_projection(
        &mut self,
        path: &Path,
        side: RawChangeSide,
        index: usize,
    ) -> Result<(), String> {
        if !self.slots.contains_key(path) {
            if self.slots.len() >= self.limits.file_limit {
                return Err("raw coverage projection path bound exceeded".to_string());
            }
            let path_bytes = native_storage_bytes(path)?;
            let retained = self
                .retained_path_bytes
                .checked_add(path_bytes)
                .filter(|bytes| *bytes <= self.limits.max_retained_path_bytes)
                .ok_or_else(|| "raw coverage retained path byte bound exceeded".to_string())?;
            // Our retained PathBuf clone follows path/count admission. The
            // parser-supplied native/decoded paths already exist; this budget
            // makes no claim about those separate parser allocations.
            self.slots.insert(path.to_path_buf(), SideCounts::default());
            self.retained_path_bytes = retained;
        }
        let counts = self
            .slots
            .get_mut(path)
            .ok_or_else(|| "raw coverage admitted projection path disappeared".to_string())?;
        let next = match side {
            RawChangeSide::Added => &mut counts.added,
            RawChangeSide::Removed => &mut counts.removed,
        };
        if index != *next {
            return Err("raw coverage duplicate, overlapping or missing insertion slot".to_string());
        }
        *next = next
            .checked_add(1)
            .ok_or_else(|| "raw coverage insertion slot overflow".to_string())?;
        Ok(())
    }

    fn into_data(self) -> Result<(LedgerSink<'saved>, RawCoverageSummary), String> {
        let facts = self
            .facts
            .ok_or_else(|| "raw coverage parser did not deliver a successful EOF".to_string())?;
        if let LedgerSink::Compare { saved, cursor } = &self.writer.sink
            && *cursor != saved.len()
        {
            return Err("raw coverage saved ledger has trailing or duplicated data".to_string());
        }
        let summary = RawCoverageSummary {
            raw_sha256: self.raw_sha256,
            ledger_sha256: format!("sha256:{:x}", self.writer.digest.finalize()),
            projection_sha256: facts.projection_sha256,
            raw_bytes: facts.end.bytes,
            ledger_bytes: self.writer.bytes,
            records: facts.end.records,
            sections: facts.end.sections,
            hunks: facts.end.hunks,
            changed_files: facts.changed_files,
            added_lines: facts.added_lines,
            removed_lines: facts.removed_lines,
        };
        Ok((self.writer.sink, summary))
    }
}

impl RawDiffObserver for LedgerObserver<'_, '_> {
    fn record(&mut self, record: RawRecord<'_>) -> Result<(), String> {
        if self.facts.is_some() || self.pending.is_some() {
            return Err("raw coverage record follows EOF or an unreduced record".to_string());
        }
        // Global LF preflight has admitted even the parser's decoded lookahead.
        // This callback independently counts actual consumed original records.
        let records = self
            .records
            .checked_add(1)
            .filter(|count| *count <= self.limits.max_records)
            .ok_or_else(|| "raw coverage record count bound exceeded".to_string())?;
        if record.ordinal != self.records || record.start != self.consumed_bytes {
            return Err("raw coverage record ordinal or interval is not contiguous".to_string());
        }
        let bytes = self.checked_record(record)?;
        self.pending = Some(PendingRecord {
            ordinal: record.ordinal,
            start: record.start,
            end: record.end,
            sha256: format!("sha256:{:x}", Sha256::digest(bytes)),
        });
        self.records = records;
        self.consumed_bytes = record.end;
        Ok(())
    }

    fn reduction(&mut self, reduction: RawReduction<'_>) -> Result<(), String> {
        if self.facts.is_some() {
            return Err("raw coverage reduction follows EOF".to_string());
        }
        let pending = self
            .pending
            .take()
            .ok_or_else(|| "raw coverage reduction has no pending original record".to_string())?;
        let bytes = self.checked_record(reduction.record)?;
        if pending.ordinal != reduction.record.ordinal
            || pending.start != reduction.record.start
            || pending.end != reduction.record.end
        {
            return Err("raw coverage reduction does not match its original record".to_string());
        }
        match reduction.kind {
            RawRecordKind::GitBoundary => {
                if reduction.section != Some(self.sections) {
                    return Err("raw coverage section ordinal is not sequential".to_string());
                }
                self.sections = self.sections.checked_add(1)
                    .ok_or_else(|| "raw coverage section count overflow".to_string())?;
            }
            RawRecordKind::HunkHeader
            | RawRecordKind::CombinedHunk
            | RawRecordKind::MalformedHunk => {
                if reduction.hunk != Some(self.hunks) {
                    return Err("raw coverage hunk ordinal is not sequential".to_string());
                }
                self.hunks = self.hunks.checked_add(1)
                    .ok_or_else(|| "raw coverage hunk count overflow".to_string())?;
            }
            _ => {}
        }
        if reduction.section.is_some_and(|section| section >= self.sections)
            || reduction.hunk.is_some_and(|hunk| hunk >= self.hunks)
        {
            return Err("raw coverage reduction references an unobserved section or hunk".into());
        }
        validate_projection_kind(reduction.kind, reduction.projection)?;
        if let Some((side, index)) = reduction.projection {
            let path = reduction
                .native_path
                .ok_or_else(|| "raw coverage insertion has no native parser path".to_string())?;
            self.note_projection(path, side, index)?;
        }
        let token = reduction
            .raw_path_token
            .map(|token| token_fact(bytes, pending.start, token))
            .transpose()?;
        let (kind, body_index, path_opened) = kind_fields(reduction.kind);
        write_entry(
            &mut self.writer,
            &RecordFact {
                tag: "record",
                ordinal: pending.ordinal,
                start: pending.start,
                end: pending.end,
                record_sha256: &pending.sha256,
                section: reduction.section,
                hunk: reduction.hunk,
                kind,
                body_index,
                path_opened,
                declared_ranges: reduction.declared_ranges,
                remaining_before: reduction.remaining_before,
                remaining_after: reduction.remaining_after,
                consumed: reduction.consumed,
                coordinates: reduction.coordinates,
                raw_path_token: token,
                decoded_path_bytes: reduction.decoded_path_bytes.as_deref(),
                native_path: reduction.native_path.map(NativePath),
                projection: reduction.projection.map(|(side, index)| ProjectionSlot {
                    side: side_name(side),
                    index,
                }),
            },
        )
    }

    fn finish(&mut self, end: RawEnd, parsed: &ParsedDiff) -> Result<(), String> {
        if self.facts.is_some() || self.pending.is_some() {
            return Err("raw coverage duplicate EOF or unreduced final record".to_string());
        }
        if end.bytes != self.raw.len()
            || end.bytes != self.consumed_bytes
            || end.records != self.records
            || end.records != self.expected_records
            || end.sections != self.sections
            || end.hunks != self.hunks
        {
            return Err("raw coverage EOF does not reconcile original consumption".to_string());
        }
        if parsed.changed_files.len() > self.limits.file_limit {
            return Err("raw coverage final projection exceeds its admitted file bound".to_string());
        }
        let mut added_lines = 0usize;
        let mut removed_lines = 0usize;
        let mut previous_path: Option<&Path> = None;
        for file in &parsed.changed_files {
            if previous_path.is_some_and(|previous| previous >= file.path.as_path()) {
                return Err("raw coverage final file ordinals are duplicate or out of order".into());
            }
            previous_path = Some(file.path.as_path());
            let counts = self.slots.remove(&file.path).unwrap_or_default();
            if counts.added != file.added_lines.len() || counts.removed != file.removed_lines.len() {
                return Err("raw coverage final side vectors do not reconcile insertion slots".into());
            }
            added_lines = added_lines
                .checked_add(counts.added)
                .ok_or_else(|| "raw coverage added-line count overflow".to_string())?;
            removed_lines = removed_lines
                .checked_add(counts.removed)
                .ok_or_else(|| "raw coverage removed-line count overflow".to_string())?;
        }
        if !self.slots.is_empty() {
            return Err("raw coverage insertion path is absent from the final projection".to_string());
        }
        let projection_sha256 =
            semantic_projection_digest(parsed, self.limits.max_projection_bytes)?;
        write_entry(
            &mut self.writer,
            &EndFact {
                tag: "eof",
                bytes: end.bytes,
                records: end.records,
                sections: end.sections,
                hunks: end.hunks,
                projection_sha256: &projection_sha256,
                files: FileCounts(&parsed.changed_files),
                metadata: Metadata(parsed),
            },
        )?;
        self.facts = Some(CoverageFacts {
            end,
            projection_sha256,
            changed_files: parsed.changed_files.len(),
            added_lines,
            removed_lines,
        });
        Ok(())
    }
}

fn write_entry(writer: &mut LedgerWriter<'_>, value: &impl Serialize) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, value)
        .map_err(|error| format!("raw coverage ledger write failed: {error}"))?;
    writer
        .write_all(b"\n")
        .map_err(|error| format!("raw coverage ledger write failed: {error}"))
}

#[derive(Serialize)]
struct TokenFact<'a> {
    start: usize,
    end: usize,
    bytes: &'a [u8],
}

fn token_fact<'a>(
    record_bytes: &'a [u8],
    record_start: usize,
    token: &[u8],
) -> Result<TokenFact<'a>, String> {
    let relative = (token.as_ptr() as usize)
        .checked_sub(record_bytes.as_ptr() as usize)
        .ok_or_else(|| "raw coverage path token is outside its winning record".to_string())?;
    let relative_end = relative
        .checked_add(token.len())
        .ok_or_else(|| "raw coverage path token span overflow".to_string())?;
    let bytes = record_bytes
        .get(relative..relative_end)
        .filter(|bytes| *bytes == token)
        .ok_or_else(|| "raw coverage path token is outside its winning record".to_string())?;
    let start = record_start
        .checked_add(relative)
        .ok_or_else(|| "raw coverage path token offset overflow".to_string())?;
    let end = start
        .checked_add(bytes.len())
        .ok_or_else(|| "raw coverage path token offset overflow".to_string())?;
    Ok(TokenFact { start, end, bytes })
}

#[derive(Serialize)]
struct ProjectionSlot {
    side: &'static str,
    index: usize,
}

#[derive(Serialize)]
struct RecordFact<'a> {
    tag: &'static str,
    ordinal: usize,
    start: usize,
    end: usize,
    record_sha256: &'a str,
    section: Option<usize>,
    hunk: Option<usize>,
    kind: &'static str,
    body_index: Option<usize>,
    path_opened: Option<bool>,
    declared_ranges: Option<((usize, usize), (usize, usize))>,
    remaining_before: Option<(usize, usize)>,
    remaining_after: Option<(usize, usize)>,
    consumed: Option<(usize, usize)>,
    coordinates: Option<(usize, usize)>,
    raw_path_token: Option<TokenFact<'a>>,
    decoded_path_bytes: Option<&'a [u8]>,
    native_path: Option<NativePath<'a>>,
    projection: Option<ProjectionSlot>,
}

#[derive(Serialize)]
struct EndFact<'a> {
    tag: &'static str,
    bytes: usize,
    records: usize,
    sections: usize,
    hunks: usize,
    projection_sha256: &'a str,
    files: FileCounts<'a>,
    metadata: Metadata<'a>,
}

fn side_name(side: RawChangeSide) -> &'static str {
    match side {
        RawChangeSide::Added => "added",
        RawChangeSide::Removed => "removed",
    }
}

fn validate_projection_kind(
    kind: RawRecordKind,
    projection: Option<(RawChangeSide, usize)>,
) -> Result<(), String> {
    match (kind, projection) {
        (RawRecordKind::Body(BodyDisposition::Added(index)), Some((RawChangeSide::Added, slot)))
        | (
            RawRecordKind::Body(BodyDisposition::Removed(index)),
            Some((RawChangeSide::Removed, slot)),
        ) if index == slot => Ok(()),
        (RawRecordKind::Body(BodyDisposition::Added(_) | BodyDisposition::Removed(_)), _)
        | (_, Some(_)) => Err("raw coverage insertion kind and projection disagree".to_string()),
        (_, None) => Ok(()),
    }
}

/// Serialization of actual winning outcomes, never a second classifier.
fn kind_fields(kind: RawRecordKind) -> (&'static str, Option<usize>, Option<bool>) {
    match kind {
        RawRecordKind::GitBoundary => ("git_boundary", None, None),
        RawRecordKind::Binary => ("binary", None, None),
        RawRecordKind::SubmoduleMode => ("submodule_mode", None, None),
        RawRecordKind::SubmoduleIndex => ("submodule_index", None, None),
        RawRecordKind::Rename => ("rename", None, None),
        RawRecordKind::PathMarker(outcome) => match outcome {
            PathMarkerOutcome::Metadata => ("path_marker_metadata", None, None),
            PathMarkerOutcome::Old => ("path_marker_old", None, None),
            PathMarkerOutcome::New { opened } => ("path_marker_new", None, Some(opened)),
            PathMarkerOutcome::RejectedNew => ("path_marker_rejected_new", None, None),
            PathMarkerOutcome::SymlinkNew => ("path_marker_symlink_new", None, None),
        },
        RawRecordKind::HunkHeader => ("hunk_header", None, None),
        RawRecordKind::CombinedHunk => ("combined_hunk", None, None),
        RawRecordKind::MalformedHunk => ("malformed_hunk", None, None),
        RawRecordKind::Body(disposition) => match disposition {
            BodyDisposition::Outside => ("body_outside", None, None),
            BodyDisposition::NoNewline => ("body_no_newline", None, None),
            BodyDisposition::ZeroCoordinate => ("body_zero_coordinate", None, None),
            BodyDisposition::NoPath => ("body_no_path", None, None),
            BodyDisposition::MissingFile => ("body_missing_file", None, None),
            BodyDisposition::Conflict => ("body_conflict", None, None),
            BodyDisposition::Added(index) => ("body_added", Some(index), None),
            BodyDisposition::Removed(index) => ("body_removed", Some(index), None),
            BodyDisposition::Context => ("body_context", None, None),
            BodyDisposition::Unknown => ("body_unknown", None, None),
            BodyDisposition::CoordinateOverflow => ("body_coordinate_overflow", None, None),
        },
    }
}

fn native_representation() -> &'static str {
    if cfg!(unix) {
        "unix_bytes"
    } else if cfg!(windows) {
        "windows_utf16"
    } else {
        "encoded_os_str"
    }
}

fn native_storage_bytes(path: &Path) -> Result<usize, String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        let wide_bytes = path
            .as_os_str()
            .encode_wide()
            .count()
            .checked_mul(2)
            .ok_or_else(|| "raw coverage native path byte count overflow".to_string())?;
        Ok(wide_bytes.max(path.as_os_str().as_encoded_bytes().len()))
    }
    #[cfg(not(windows))]
    {
        Ok(path.as_os_str().as_encoded_bytes().len())
    }
}

/// Native identity is encoded only from the actual parser Path. No string,
/// byte token or saved native encoding is ever inverted into a lookup PathBuf.
struct NativePath<'a>(&'a Path);

impl Serialize for NativePath<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("NativePath", 2)?;
        state.serialize_field("representation", native_representation())?;
        #[cfg(windows)]
        state.serialize_field("units", &WidePath(self.0))?;
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            state.serialize_field("units", self.0.as_os_str().as_bytes())?;
        }
        #[cfg(not(any(unix, windows)))]
        state.serialize_field("units", self.0.as_os_str().as_encoded_bytes())?;
        state.end()
    }
}

#[cfg(windows)]
struct WidePath<'a>(&'a Path);

#[cfg(windows)]
impl Serialize for WidePath<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use std::os::windows::ffi::OsStrExt;
        let mut seq = serializer.serialize_seq(Some(self.0.as_os_str().encode_wide().count()))?;
        for unit in self.0.as_os_str().encode_wide() {
            seq.serialize_element(&unit)?;
        }
        seq.end()
    }
}

struct Projection<'a>(&'a ParsedDiff);

impl Serialize for Projection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let parsed = self.0;
        let mut state = serializer.serialize_struct("ParsedDiffProjection", 2)?;
        state.serialize_field("changed_files", &Files(&parsed.changed_files))?;
        state.serialize_field("metadata", &Metadata(parsed))?;
        state.end()
    }
}

struct Files<'a>(&'a [ChangedFile]);

impl Serialize for Files<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for file in self.0 {
            seq.serialize_element(&FileProjection(file))?;
        }
        seq.end()
    }
}

struct FileProjection<'a>(&'a ChangedFile);

impl Serialize for FileProjection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("ChangedFileProjection", 3)?;
        state.serialize_field("path", &NativePath(&self.0.path))?;
        state.serialize_field("added_lines", &Lines(&self.0.added_lines))?;
        state.serialize_field("removed_lines", &Lines(&self.0.removed_lines))?;
        state.end()
    }
}

struct Lines<'a>(&'a [ChangedLine]);

impl Serialize for Lines<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for line in self.0 {
            seq.serialize_element(&LineProjection(line))?;
        }
        seq.end()
    }
}

struct LineProjection<'a>(&'a ChangedLine);

impl Serialize for LineProjection<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("ChangedLineProjection", 3)?;
        state.serialize_field("line", &self.0.line)?;
        state.serialize_field("new_side_line", &self.0.new_side_line)?;
        state.serialize_field("text", &self.0.text)?;
        state.end()
    }
}

#[derive(Serialize)]
struct FileCount<'a> {
    ordinal: usize,
    path: NativePath<'a>,
    added_lines: usize,
    removed_lines: usize,
}

struct FileCounts<'a>(&'a [ChangedFile]);

impl Serialize for FileCounts<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for (ordinal, file) in self.0.iter().enumerate() {
            seq.serialize_element(&FileCount {
                ordinal,
                path: NativePath(&file.path),
                added_lines: file.added_lines.len(),
                removed_lines: file.removed_lines.len(),
            })?;
        }
        seq.end()
    }
}

struct Paths<'a>(&'a [PathBuf]);

impl Serialize for Paths<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for path in self.0 {
            seq.serialize_element(&NativePath(path))?;
        }
        seq.end()
    }
}

struct Metadata<'a>(&'a ParsedDiff);

impl Serialize for Metadata<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let parsed = self.0;
        let mut state = serializer.serialize_struct("ParsedDiffMetadata", 8)?;
        state.serialize_field("deleted_file_count", &parsed.deleted_file_count)?;
        state.serialize_field("submodule_file_count", &parsed.submodule_file_count)?;
        state.serialize_field("renamed_file_count", &parsed.renamed_file_count)?;
        state.serialize_field("pure_rename_file_count", &parsed.pure_rename_file_count)?;
        state.serialize_field("pure_rename_paths", &Paths(&parsed.pure_rename_paths))?;
        state.serialize_field("truncated_file_sections", &parsed.truncated_file_sections)?;
        state.serialize_field("raw_line1_bom_paths", &Paths(&parsed.raw_line1_bom_paths))?;
        state.serialize_field("limitations", &parsed.limitations)?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::diff::parse::parse_unified_diff_with_metadata;
    use serde_json::Value;

    const FIRST: &str = "diff --git a/src/a.rs b/src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1 +1 @@\n-old\n+new\n";

    fn limits() -> RawCoverageLimits {
        RawCoverageLimits {
            file_limit: 16,
            max_raw_bytes: 1024 * 1024,
            max_records: 1024,
            max_ledger_bytes: 1024 * 1024,
            max_retained_path_bytes: 16 * 1024,
            max_projection_bytes: 1024 * 1024,
        }
    }

    fn assert_projection_eq(actual: &ParsedDiff, expected: &ParsedDiff) {
        assert_eq!(actual.changed_files.len(), expected.changed_files.len());
        for (actual, expected) in actual.changed_files.iter().zip(&expected.changed_files) {
            assert_eq!(actual.path, expected.path);
            assert_eq!(actual.added_lines, expected.added_lines);
            assert_eq!(actual.removed_lines, expected.removed_lines);
        }
        assert_eq!(actual.deleted_file_count, expected.deleted_file_count);
        assert_eq!(actual.submodule_file_count, expected.submodule_file_count);
        assert_eq!(actual.renamed_file_count, expected.renamed_file_count);
        assert_eq!(actual.pure_rename_file_count, expected.pure_rename_file_count);
        assert_eq!(actual.pure_rename_paths, expected.pure_rename_paths);
        assert_eq!(actual.truncated_file_sections, expected.truncated_file_sections);
        assert_eq!(actual.raw_line1_bom_paths, expected.raw_line1_bom_paths);
        assert_eq!(actual.limitations, expected.limitations);
    }

    fn ledger_entries(ledger: &[u8]) -> Result<Vec<Value>, String> {
        ledger
            .split(|byte| *byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_slice(line).map_err(|error| error.to_string()))
            .collect()
    }

    fn assert_saved_refusal(raw: &[u8], ledger: &[u8]) -> Result<(), String> {
        let error = verify_raw_coverage(raw, ledger, limits())
            .err()
            .ok_or("corrupted raw coverage ledger was accepted")?;
        assert!(error.starts_with("raw coverage"), "{error}");
        Ok(())
    }

    // Patch only the changed top-level value span. Re-serializing a whole
    // Value object would reorder unrelated struct fields and weaken controls.
    fn mutate_entry(
        ledger: &[u8],
        ordinal: usize,
        mutate: impl FnOnce(&mut Value),
    ) -> Result<Vec<u8>, String> {
        let mut output = Vec::new();
        let mut mutate = Some(mutate);
        for (index, line) in ledger.split_inclusive(|byte| *byte == b'\n').enumerate() {
            if index != ordinal {
                output.extend_from_slice(line);
                continue;
            }
            let mut value: Value =
                serde_json::from_slice(line).map_err(|error| error.to_string())?;
            let original = value.clone();
            let apply = mutate.take().ok_or("test entry mutation was applied twice")?;
            apply(&mut value);
            let object = value.as_object().ok_or("test entry is not an object")?;
            let mut field = None;
            for (key, current) in object {
                if original.get(key) != Some(current) {
                    if field.is_some() {
                        return Err("test mutation changed more than one root field".to_string());
                    }
                    field = Some(key.as_str());
                }
            }
            let Some(field) = field else {
                output.extend_from_slice(line);
                continue;
            };
            let text = std::str::from_utf8(line).map_err(|error| error.to_string())?;
            let marker = format!("\"{field}\":");
            let start = text.find(&marker).ok_or("test field span is missing")? + marker.len();
            let end = json_value_end(line, start)?;
            output.extend_from_slice(&line[..start]);
            let replacement = object.get(field).ok_or("test replacement field is missing")?;
            output.extend(serde_json::to_vec(replacement).map_err(|error| error.to_string())?);
            output.extend_from_slice(&line[end..]);
        }
        if mutate.is_some() {
            return Err("test entry mutation did not locate its target".to_string());
        }
        Ok(output)
    }

    fn json_value_end(bytes: &[u8], start: usize) -> Result<usize, String> {
        let first = *bytes.get(start).ok_or("test JSON value is missing")?;
        let container = matches!(first, b'{' | b'[');
        let quoted = first == b'"';
        let mut depth = 0usize;
        let mut in_string = false;
        let mut escaped = false;
        for (index, byte) in bytes.iter().copied().enumerate().skip(start) {
            if in_string {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == b'"' {
                    in_string = false;
                    if quoted && depth == 0 {
                        return Ok(index + 1);
                    }
                }
                continue;
            }
            match byte {
                b'"' => in_string = true,
                b'{' | b'[' => {
                    depth = depth.checked_add(1).ok_or("test JSON depth overflow")?;
                }
                b'}' | b']' if depth > 0 => {
                    depth -= 1;
                    if container && depth == 0 {
                        return Ok(index + 1);
                    }
                }
                b',' | b'}' if !container && depth == 0 => return Ok(index),
                _ => {}
            }
        }
        Err("test JSON value is unfinished".to_string())
    }

    #[test]
    fn actual_coalescing_preserves_occurrences_slots_and_both_coordinates() -> Result<(), String> {
        let input = format!(
            "{FIRST}diff --git a/src/a.rs b/src/a.rs\n--- a/./src/a.rs\n+++ b/./src/a.rs\n@@ -5 +7 @@\n-before\n+after\ndiff --git a/src/old.rs b/src/a.rs\nsimilarity index 80%\nrename from src/old.rs\nrename to src/a.rs\n--- a/src/old.rs\n+++ b/src/a.rs\n@@ -9 +11 @@\n-previous\n+current\n"
        );
        let (parsed, coverage) = build_raw_coverage(input.as_bytes(), limits())?;
        assert_projection_eq(&parsed, &parse_unified_diff_with_metadata(&input));
        assert_eq!(parsed.changed_files.len(), 1);
        assert_eq!(coverage.summary().sections, 3);
        assert_eq!(coverage.summary().added_lines, 3);
        assert_eq!(coverage.summary().removed_lines, 3);
        assert_eq!(parsed.changed_files[0].removed_lines[1].line, 5);
        assert_eq!(parsed.changed_files[0].removed_lines[1].new_side_line, 7);
        let entries = ledger_entries(coverage.ledger_bytes())?;
        let slots: Vec<_> = entries
            .iter()
            .filter(|entry| entry["kind"] == "body_added")
            .map(|entry| entry["projection"]["index"].clone())
            .collect();
        assert_eq!(slots, vec![Value::from(0), Value::from(1), Value::from(2)]);
        let (replayed, summary) =
            verify_raw_coverage(input.as_bytes(), coverage.ledger_bytes(), limits())?;
        assert_projection_eq(&replayed, &parsed);
        assert_eq!(&summary, coverage.summary());
        Ok(())
    }

    #[test]
    fn raw_crlf_invalid_text_and_native_quoted_paths_are_lossless_data() -> Result<(), String> {
        let mut input = b"--- /dev/null\r\n+++ \"b/src/raw_\\377.rs\"\r\n@@ -0,0 +1 @@\r\n+".to_vec();
        input.extend_from_slice(b"\xff\r\n--- /dev/null\r\n+++ \"b/src/raw_\\\\377.rs\"\r\n@@ -0,0 +1 @@\r\n+literal\r\n");
        let (parsed, coverage) = build_raw_coverage(&input, limits())?;
        let ordinary = parse_unified_diff_with_metadata(&String::from_utf8_lossy(&input));
        assert_projection_eq(&parsed, &ordinary);
        let entries = ledger_entries(coverage.ledger_bytes())?;
        assert_eq!(entries[0]["native_representation"], native_representation());
        let decoded: Vec<_> = entries
            .iter()
            .filter(|entry| entry["kind"] == "path_marker_new")
            .map(|entry| entry["decoded_path_bytes"].clone())
            .collect();
        assert_eq!(decoded.len(), 2);
        assert_ne!(decoded[0], decoded[1]);
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            assert_eq!(parsed.changed_files.len(), 2);
            let paths: Vec<_> = parsed
                .changed_files
                .iter()
                .map(|file| file.path.as_os_str().as_bytes())
                .collect();
            assert!(paths.iter().any(|path| path.contains(&0xff)), "{paths:?}");
            assert_ne!(paths[0], paths[1]);
        }
        #[cfg(windows)]
        assert_eq!(
            parsed.changed_files.len(),
            1,
            "the existing Windows invalid-byte octal-residue projection is preserved"
        );
        let (replayed, summary) = verify_raw_coverage(&input, coverage.ledger_bytes(), limits())?;
        assert_projection_eq(&replayed, &parsed);
        assert_eq!(&summary, coverage.summary());
        assert_eq!(coverage.summary().raw_bytes, input.len());
        Ok(())
    }

    #[test]
    fn limitations_unanchored_sections_and_no_probe_metadata_survive() -> Result<(), String> {
        let input = concat!(
            "diff --cc src/a.rs\n--- a/src/a.rs\n+++ b/src/a.rs\n@@@ -1 -1 +1 @@@\n++hidden\n",
            "--- a/src/b.rs\n+++ b/src/b.rs\n@@ -0,0 +1,4 @@\n+<<<<<<< ours\n+hidden\n+>>>>>>> theirs\n+visible\n",
            "--- a/src/c.rs\n+++ b/src/c.rs\n@@ malformed @@\n-ignored\n+ignored\n",
            "diff --git a/src/old.rs b/src/new.rs\nsimilarity index 100%\nrename from src/old.rs\nrename to src/new.rs\n",
            "diff --git a/src/gone.rs b/src/gone.rs\ndeleted file mode 100644\n--- a/src/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-gone\n",
            "diff --git a/src/blob.bin b/src/blob.bin\nBinary files a/src/blob.bin and /dev/null differ\n",
            "diff --git a/vendor/lib b/vendor/lib\nindex 1111111..2222222 160000\n--- a/vendor/lib\n+++ b/vendor/lib\n@@ -1 +1 @@\n-Subproject commit 1111111\n+Subproject commit 2222222\n",
            "--- a/src/truncated.rs\n+++ b/src/truncated.rs\n",
        );
        let (parsed, coverage) = build_raw_coverage(input.as_bytes(), limits())?;
        assert_projection_eq(&parsed, &parse_unified_diff_with_metadata(input));
        assert_eq!(parsed.deleted_file_count, 2);
        assert_eq!(parsed.pure_rename_file_count, 1);
        assert_eq!(parsed.submodule_file_count, 1);
        assert!(!parsed.limitations.is_empty(), "{parsed:?}");
        assert!(parsed.truncated_file_sections > 0, "{parsed:?}");
        let entries = ledger_entries(coverage.ledger_bytes())?;
        for kind in ["combined_hunk", "body_conflict", "malformed_hunk", "binary", "rename"] {
            assert!(entries.iter().any(|entry| entry["kind"] == kind), "{kind}");
        }
        assert!(
            entries.iter().any(|entry| {
                entry["tag"] == "record"
                    && entry["section"].is_null()
                    && entry["kind"] == "path_marker_new"
            }),
            "plain boundaries must retain unanchored section identity"
        );
        let (replayed, summary) =
            verify_raw_coverage(input.as_bytes(), coverage.ledger_bytes(), limits())?;
        assert_projection_eq(&replayed, &parsed);
        assert_eq!(&summary, coverage.summary());
        Ok(())
    }

    #[test]
    fn unknown_body_rejected_and_symlink_paths_keep_actual_winning_states() -> Result<(), String> {
        let input = concat!(
            "--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,2 +1,2 @@\n context\n-old\n+new\n",
            "\\ No newline at end of file\n@@ -5 +5 @@\n~unknown\n-before\n+after\n",
            "--- a/../escape.rs\n+++ b/../escape.rs\n@@ -1 +1 @@\n-old\n+new\n",
            "diff --git a/link.rs b/link.rs\nnew file mode 120000\n",
            "--- /dev/null\n+++ b/link.rs\n@@ -0,0 +1 @@\n+target\n",
        );
        let (parsed, coverage) = build_raw_coverage(input.as_bytes(), limits())?;
        assert_projection_eq(&parsed, &parse_unified_diff_with_metadata(input));
        let entries = ledger_entries(coverage.ledger_bytes())?;
        for kind in [
            "body_context",
            "body_no_newline",
            "body_unknown",
            "path_marker_rejected_new",
            "path_marker_symlink_new",
        ] {
            assert!(entries.iter().any(|entry| entry["kind"] == kind), "{kind}: {entries:?}");
        }
        assert!(
            entries.iter().any(|entry| {
                entry["kind"] == "path_marker_rejected_new"
                    && !entry["decoded_path_bytes"].is_null()
                    && entry["projection"].is_null()
            }),
            "decoded rejected paths must remain data without an insertion"
        );
        let (replayed, _) =
            verify_raw_coverage(input.as_bytes(), coverage.ledger_bytes(), limits())?;
        assert_projection_eq(&replayed, &parsed);
        Ok(())
    }

    #[test]
    fn saved_raw_record_path_slot_and_eof_mutations_refuse() -> Result<(), String> {
        let (_, coverage) = build_raw_coverage(FIRST.as_bytes(), limits())?;
        let entries = ledger_entries(coverage.ledger_bytes())?;
        let marker = entries
            .iter()
            .position(|entry| entry["kind"] == "path_marker_new")
            .ok_or("test new-path reduction missing")?;
        let added = entries
            .iter()
            .position(|entry| entry["kind"] == "body_added")
            .ok_or("test added insertion missing")?;
        let unchanged = mutate_entry(coverage.ledger_bytes(), added, |_| {})?;
        assert_eq!(unchanged, coverage.ledger_bytes());
        verify_raw_coverage(FIRST.as_bytes(), &unchanged, limits())?;
        for field in ["ordinal", "start", "end", "record_sha256", "section", "hunk"] {
            let changed = mutate_entry(coverage.ledger_bytes(), added, |entry| {
                entry[field] = Value::from("stale");
            })?;
            assert_saved_refusal(FIRST.as_bytes(), &changed)?;
        }
        for field in ["raw_path_token", "decoded_path_bytes", "native_path"] {
            let changed = mutate_entry(coverage.ledger_bytes(), marker, |entry| {
                entry[field] = Value::Null;
            })?;
            assert_saved_refusal(FIRST.as_bytes(), &changed)?;
        }
        let changed = mutate_entry(coverage.ledger_bytes(), added, |entry| {
            entry["projection"]["index"] = Value::from(0xffff);
        })?;
        assert_saved_refusal(FIRST.as_bytes(), &changed)?;
        let eof = entries.len() - 1;
        for field in ["records", "sections", "hunks", "projection_sha256", "files", "metadata"] {
            let changed = mutate_entry(coverage.ledger_bytes(), eof, |entry| {
                entry[field] = Value::Null;
            })?;
            assert_saved_refusal(FIRST.as_bytes(), &changed)?;
        }
        let mut changed_raw = FIRST.as_bytes().to_vec();
        let last = changed_raw
            .iter_mut()
            .find(|byte| **byte == b'o')
            .ok_or("test raw mutation target missing")?;
        *last = b'x';
        assert_saved_refusal(&changed_raw, coverage.ledger_bytes())?;
        Ok(())
    }

    #[test]
    fn missing_duplicate_overlap_and_trailing_ledger_data_refuse() -> Result<(), String> {
        let (_, coverage) = build_raw_coverage(FIRST.as_bytes(), limits())?;
        let lines: Vec<_> = coverage
            .ledger_bytes()
            .split_inclusive(|byte| *byte == b'\n')
            .collect();
        let mut missing = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            if index != 2 {
                missing.extend_from_slice(line);
            }
        }
        assert_saved_refusal(FIRST.as_bytes(), &missing)?;
        let mut duplicate = Vec::new();
        for (index, line) in lines.iter().enumerate() {
            duplicate.extend_from_slice(line);
            if index == 2 {
                duplicate.extend_from_slice(line);
            }
        }
        assert_saved_refusal(FIRST.as_bytes(), &duplicate)?;
        let overlap = mutate_entry(coverage.ledger_bytes(), 2, |entry| {
            entry["start"] = Value::from(0);
        })?;
        assert_saved_refusal(FIRST.as_bytes(), &overlap)?;
        let mut trailing = coverage.ledger_bytes().to_vec();
        trailing.extend_from_slice(lines.last().ok_or("test EOF line missing")?);
        assert_saved_refusal(FIRST.as_bytes(), &trailing)?;
        assert_saved_refusal(FIRST.as_bytes(), &[])?;
        Ok(())
    }

    #[test]
    fn byte_record_path_projection_and_existing_file_bounds_refuse() -> Result<(), String> {
        let (_, coverage) = build_raw_coverage(FIRST.as_bytes(), limits())?;
        let mut exact = limits();
        exact.max_raw_bytes = FIRST.len();
        exact.max_records = coverage.summary().records;
        exact.max_ledger_bytes = coverage.ledger_bytes().len();
        exact.max_retained_path_bytes = native_storage_bytes(Path::new("src/a.rs"))?;
        let (_, exact_coverage) = build_raw_coverage(FIRST.as_bytes(), exact)?;
        assert_eq!(exact_coverage.summary(), coverage.summary());
        for (bound, expected) in [
            ("raw", "input exceeds"),
            ("record", "record count bound"),
            ("ledger", "ledger byte bound"),
            ("path", "retained path byte bound"),
            ("projection", "projection byte bound"),
        ] {
            let mut smaller = exact;
            match bound {
                "raw" => smaller.max_raw_bytes -= 1,
                "record" => smaller.max_records -= 1,
                "ledger" => smaller.max_ledger_bytes -= 1,
                "path" => smaller.max_retained_path_bytes -= 1,
                "projection" => smaller.max_projection_bytes = 1,
                _ => return Err("unknown test bound".to_string()),
            }
            let error = build_raw_coverage(FIRST.as_bytes(), smaller)
                .err()
                .ok_or("under-admitted coverage returned data")?;
            assert!(error.contains(expected), "{bound}: {error}");
        }
        let input = format!(
            "{FIRST}--- /dev/null\n+++ b/src/b.rs\n@@ -0,0 +1 @@\n+second\n"
        );
        let mut one_file = limits();
        one_file.file_limit = 1;
        let error = build_raw_coverage(input.as_bytes(), one_file)
            .err()
            .ok_or("existing parser file-limit refusal became ledger success")?;
        assert!(error.starts_with("diff_scope_oversized:"), "{error}");
        Ok(())
    }

    #[test]
    fn helper_owned_record_sequencing_and_single_eof_are_required() -> Result<(), String> {
        let raw = b"+line\n";
        let mut observer = LedgerObserver::new(raw, limits(), LedgerSink::Build(Vec::new()))?;
        let detached = raw.to_vec();
        let error = observer
            .record(RawRecord { ordinal: 0, start: 0, end: raw.len(), bytes: &detached })
            .err()
            .ok_or("detached equal-byte record was accepted")?;
        assert!(error.contains("helper-owned original subslice"), "{error}");
        let record = RawRecord { ordinal: 0, start: 0, end: raw.len(), bytes: raw };
        observer.record(record)?;
        let error = observer.record(record).err().ok_or("unreduced duplicate record was accepted")?;
        assert!(error.contains("unreduced record"), "{error}");
        let error = observer
            .finish(
                RawEnd { bytes: raw.len(), records: 1, sections: 0, hunks: 0 },
                &ParsedDiff::default(),
            )
            .err()
            .ok_or("EOF with an unreduced record was accepted")?;
        assert!(error.contains("unreduced final record"), "{error}");
        let mut empty = LedgerObserver::new(b"", limits(), LedgerSink::Build(Vec::new()))?;
        let end = RawEnd { bytes: 0, records: 0, sections: 0, hunks: 0 };
        let error = empty
            .finish(
                RawEnd { sections: 1, ..end },
                &ParsedDiff::default(),
            )
            .err()
            .ok_or("unobserved section count was accepted at EOF")?;
        assert!(error.contains("EOF does not reconcile"), "{error}");
        empty.finish(end, &ParsedDiff::default())?;
        let error = empty.finish(end, &ParsedDiff::default())
            .err().ok_or("duplicate EOF was accepted")?;
        assert!(error.contains("duplicate EOF"), "{error}");
        let mut slots = LedgerObserver::new(b"", limits(), LedgerSink::Build(Vec::new()))?;
        slots.note_projection(Path::new("a.rs"), RawChangeSide::Added, 0)?;
        let duplicate = slots.note_projection(Path::new("a.rs"), RawChangeSide::Added, 0)
            .err().ok_or("duplicate insertion slot was accepted")?;
        assert!(duplicate.contains("duplicate, overlapping or missing"), "{duplicate}");
        let skipped = slots.note_projection(Path::new("a.rs"), RawChangeSide::Added, 2)
            .err().ok_or("missing insertion slot was accepted")?;
        assert!(skipped.contains("duplicate, overlapping or missing"), "{skipped}");
        let mismatch = validate_projection_kind(
            RawRecordKind::Body(BodyDisposition::Added(0)),
            Some((RawChangeSide::Removed, 0)),
        ).err().ok_or("changed insertion side was accepted")?;
        assert!(mismatch.contains("kind and projection disagree"), "{mismatch}");
        Ok(())
    }

    #[test]
    fn full_projection_digest_binds_text_coordinates_paths_and_every_metadata_field(
    ) -> Result<(), String> {
        let baseline = parse_unified_diff_with_metadata(FIRST);
        let expected = semantic_projection_digest(&baseline, limits().max_projection_bytes)?;
        let malformed = parse_unified_diff_with_metadata(
            "--- a/a.rs\n+++ b/a.rs\n@@ malformed @@\n+ignored\n",
        );
        let limitation = malformed
            .limitations
            .first()
            .cloned()
            .ok_or("actual malformed grammar did not report a limitation")?;
        for mutation in 0..16 {
            let mut changed = parse_unified_diff_with_metadata(FIRST);
            match mutation {
                0 => changed.changed_files[0].path = PathBuf::from("different.rs"),
                1 => changed.changed_files[0].added_lines[0].text.push('x'),
                2 => changed.changed_files[0].added_lines[0].line += 1,
                3 => changed.changed_files[0].added_lines[0].new_side_line += 1,
                4 => changed.changed_files[0].removed_lines[0].text.push('x'),
                5 => changed.changed_files[0].removed_lines[0].line += 1,
                6 => changed.changed_files[0].removed_lines[0].new_side_line += 1,
                7 => changed.deleted_file_count += 1,
                8 => changed.submodule_file_count += 1,
                9 => changed.renamed_file_count += 1,
                10 => changed.pure_rename_file_count += 1,
                11 => changed.pure_rename_paths.push(PathBuf::from("pure.rs")),
                12 => changed.truncated_file_sections += 1,
                13 => changed.raw_line1_bom_paths.push(PathBuf::from("bom.rs")),
                14 => changed.limitations.push(limitation.clone()),
                15 => changed.changed_files[0].added_lines.clear(),
                _ => return Err("unknown projection mutation".to_string()),
            }
            assert_ne!(
                semantic_projection_digest(&changed, limits().max_projection_bytes)?,
                expected,
                "projection mutation {mutation} escaped its commitment"
            );
        }
        Ok(())
    }

    #[test]
    fn empty_and_zero_file_metadata_subjects_do_not_invent_eligibility() -> Result<(), String> {
        let mut zero = limits();
        zero.file_limit = 0;
        zero.max_records = 0;
        zero.max_retained_path_bytes = 0;
        let (parsed, coverage) = build_raw_coverage(b"", zero)?;
        assert!(parsed.changed_files.is_empty(), "{parsed:?}");
        assert_eq!(coverage.summary().records, 0);
        let (_, replayed) = verify_raw_coverage(b"", coverage.ledger_bytes(), zero)?;
        assert_eq!(&replayed, coverage.summary());
        let input = "diff --git a/gone.rs b/gone.rs\ndeleted file mode 100644\n--- a/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-gone\n";
        zero.max_records = 64;
        let (metadata, coverage) = build_raw_coverage(input.as_bytes(), zero)?;
        assert_eq!(metadata.deleted_file_count, 1);
        assert!(metadata.changed_files.is_empty(), "{metadata:?}");
        assert_eq!(coverage.summary().added_lines, 0);
        assert_eq!(coverage.summary().removed_lines, 0);
        let (replayed, _) = verify_raw_coverage(input.as_bytes(), coverage.ledger_bytes(), zero)?;
        assert_projection_eq(&replayed, &metadata);
        Ok(())
    }
}
