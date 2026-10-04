//! Producer-owned numeric diagnostic origins for current Rust findings (#4464).
//!
//! Parser byte intervals travel with the seeded probe through final-ID
//! assignment. Exact geometry is admitted only when the captured analysis
//! bytes, cached file facts, candidate-current disposition, and the parser
//! span all agree. Projection later selects these numbers; it does not search
//! source text or reread the filesystem.
//!
//! Every current Rust finding in this map receives a record. Missing parser
//! span, mismatched facts, lexical fallback, and other refusals stay coarse
//! zero-width. Absent or default-empty maps, and other languages, keep the
//! saved-line heuristic. Non-current records stay bounded so a deleted
//! expression cannot paint a current line.
//!
//! Captured files keep the decoder `Cow`: valid UTF-8, including a supported
//! leading BOM strip, is borrowed from the loaded buffer; only a required
//! lossy decode owns. UTF-8/16/32 scalar-boundary coordinates are built once
//! per file and admitted endpoints are derived from that index.

use super::facts::rust_source_text;
use super::rust_index::RustIndex;
use crate::domain::Finding;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Matches the LSP adapter's `MAX_LINE_SPAN_WIDTH`. This module cannot
/// import that adapter; position tests pin the two constants together.
pub(crate) const ORIGIN_MAX_SPAN_WIDTH: u32 = 120;

/// Parser-owned start inside the captured source. End is derived from the
/// finding expression only when that expression is a contiguous slice at this
/// offset (after the same leading trim the probe producer already applied).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ParserByteSpan {
    pub start_byte: usize,
}

impl ParserByteSpan {
    /// Same-line parser geometry only. A newline means the producer owns a
    /// changed full line, not an exact expression slice.
    pub(crate) fn same_line(text: &str, start_byte: usize) -> Option<Self> {
        if text.contains('\n') {
            None
        } else {
            Some(Self { start_byte })
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OriginKind {
    Exact,
    CoarseZeroWidth,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EncodedSpan {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct EncodedOrigin {
    /// 0-based LSP line.
    pub line: u32,
    pub utf8: EncodedSpan,
    pub utf16: EncodedSpan,
    pub utf32: EncodedSpan,
    pub kind: OriginKind,
}

impl EncodedOrigin {
    pub(crate) fn for_utf8(&self) -> EncodedSpan {
        self.utf8
    }

    pub(crate) fn for_utf16(&self) -> EncodedSpan {
        self.utf16
    }

    pub(crate) fn for_utf32(&self) -> EncodedSpan {
        self.utf32
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct RustDiagnosticOrigins {
    records: BTreeMap<String, EncodedOrigin>,
}

impl RustDiagnosticOrigins {
    pub(crate) fn get(&self, probe_id: &str) -> Option<&EncodedOrigin> {
        self.records.get(probe_id)
    }

    pub(crate) fn for_finding(&self, finding: &Finding) -> Option<&EncodedOrigin> {
        self.get(&finding.id)
            .or_else(|| self.get(finding.probe.id.0.as_str()))
    }

    pub(crate) fn insert(&mut self, probe_id: String, origin: EncodedOrigin) {
        self.records.insert(probe_id, origin);
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// Inputs already consumed by the Rust adapter for one analysis pass.
pub(crate) struct OriginBuildContext<'a> {
    pub root: &'a Path,
    pub loaded_files: &'a [(PathBuf, Vec<u8>)],
    pub index: &'a RustIndex,
    pub parser_spans: &'a BTreeMap<String, ParserByteSpan>,
}

struct CapturedFile<'a> {
    source: Cow<'a, str>,
    has_standalone_cr: bool,
    lines: Vec<LineSpan>,
    coords: ScalarIndex,
    facts_match: bool,
    used_lexical_fallback: bool,
}

/// Scalar-boundary UTF-8/16/32 coordinates for one captured file.
/// Offsets are file-absolute; line-relative LSP endpoints subtract the line start.
struct ScalarIndex {
    offsets: Vec<usize>,
    utf16: Vec<u32>,
    utf32: Vec<u32>,
}

impl ScalarIndex {
    fn build(source: &str) -> Self {
        let mut offsets = Vec::new();
        let mut utf16 = Vec::new();
        let mut utf32 = Vec::new();
        offsets.push(0);
        utf16.push(0);
        utf32.push(0);
        let mut byte = 0usize;
        let mut u16_units = 0u32;
        let mut u32_units = 0u32;
        for ch in source.chars() {
            byte = byte.saturating_add(ch.len_utf8());
            u16_units = u16_units.saturating_add(ch.len_utf16() as u32);
            u32_units = u32_units.saturating_add(1);
            offsets.push(byte);
            utf16.push(u16_units);
            utf32.push(u32_units);
        }
        Self {
            offsets,
            utf16,
            utf32,
        }
    }

    fn index_of(&self, byte: usize) -> Option<usize> {
        self.offsets.binary_search(&byte).ok()
    }

    fn units(&self, byte: usize, encoding: Encoding) -> Option<u32> {
        let i = self.index_of(byte)?;
        match encoding {
            Encoding::Utf8 => u32::try_from(byte).ok(),
            Encoding::Utf16 => self.utf16.get(i).copied(),
            Encoding::Utf32 => self.utf32.get(i).copied(),
        }
    }

    fn capped_end(&self, start: usize, end: usize, encoding: Encoding) -> Option<u32> {
        let start_i = self.index_of(start)?;
        let end_i = self.index_of(end)?;
        let start_units = match encoding {
            Encoding::Utf8 => u32::try_from(start).ok()?,
            Encoding::Utf16 => *self.utf16.get(start_i)?,
            Encoding::Utf32 => *self.utf32.get(start_i)?,
        };
        let mut last = start_units;
        let mut i = start_i.saturating_add(1);
        while i <= end_i {
            let abs = match encoding {
                Encoding::Utf8 => u32::try_from(*self.offsets.get(i)?).ok()?,
                Encoding::Utf16 => *self.utf16.get(i)?,
                Encoding::Utf32 => *self.utf32.get(i)?,
            };
            if abs.saturating_sub(start_units) > ORIGIN_MAX_SPAN_WIDTH {
                break;
            }
            last = abs;
            i = i.saturating_add(1);
        }
        Some(last)
    }

    fn line_span(
        &self,
        line_start: usize,
        expr_start: usize,
        expr_end: usize,
        encoding: Encoding,
    ) -> Option<EncodedSpan> {
        let origin = self.units(line_start, encoding)?;
        let start = self.units(expr_start, encoding)?.saturating_sub(origin);
        let end = self
            .capped_end(expr_start, expr_end, encoding)?
            .saturating_sub(origin);
        Some(EncodedSpan { start, end })
    }
}

pub(crate) fn origins_for_rust_findings(
    findings: &[Finding],
    context: &OriginBuildContext<'_>,
) -> RustDiagnosticOrigins {
    let captured = captured_file_index(context.loaded_files, context.index);
    let mut origins = RustDiagnosticOrigins::default();
    for finding in findings {
        let origin = match relative_finding_path(context.root, finding) {
            Some(relative) => origin_for_finding(
                finding,
                parser_span_for_finding(finding, context.parser_spans),
                captured.get(&relative),
            ),
            None => missing_input_origin(),
        };
        origins.insert(finding.id.clone(), origin);
    }
    origins
}

fn captured_file_index<'a>(
    loaded_files: &'a [(PathBuf, Vec<u8>)],
    index: &RustIndex,
) -> BTreeMap<PathBuf, CapturedFile<'a>> {
    let mut captured = BTreeMap::new();
    for (path, bytes) in loaded_files {
        let source = rust_source_text(bytes).text;
        let facts = index.files().get(path);
        let text = source.as_ref();
        captured.insert(
            path.clone(),
            CapturedFile {
                has_standalone_cr: has_standalone_cr(text),
                lines: lsp_lines(text),
                coords: ScalarIndex::build(text),
                facts_match: facts.is_some_and(|facts| facts.source == text),
                used_lexical_fallback: facts.is_some_and(|facts| facts.used_lexical_fallback),
                source,
            },
        );
    }
    captured
}

fn parser_span_for_finding(
    finding: &Finding,
    parser_spans: &BTreeMap<String, ParserByteSpan>,
) -> Option<ParserByteSpan> {
    parser_spans
        .get(finding.id.as_str())
        .copied()
        .or_else(|| parser_spans.get(finding.probe.id.0.as_str()).copied())
}

fn relative_finding_path(root: &Path, finding: &Finding) -> Option<PathBuf> {
    let file = &finding.probe.location.file;
    if let Ok(relative) = file.strip_prefix(root) {
        return Some(relative.to_path_buf());
    }
    if file.is_relative() {
        return Some(file.clone());
    }
    None
}

fn origin_for_finding(
    finding: &Finding,
    span: Option<ParserByteSpan>,
    captured: Option<&CapturedFile<'_>>,
) -> EncodedOrigin {
    let Some(captured) = captured else {
        return missing_input_origin();
    };
    if !finding.source_currentness.permits_candidate_action() {
        return coarse_on_line(&captured.lines, finding.probe.location.line);
    }
    let Some(span) = span else {
        return coarse_on_line(&captured.lines, finding.probe.location.line);
    };
    if captured.has_standalone_cr {
        return missing_input_origin();
    }
    if captured.used_lexical_fallback || !captured.facts_match {
        return coarse_on_line(&captured.lines, finding.probe.location.line);
    }
    exact_origin(captured, span, finding)
        .unwrap_or_else(|| coarse_on_line(&captured.lines, finding.probe.location.line))
}

fn missing_input_origin() -> EncodedOrigin {
    EncodedOrigin {
        line: 0,
        utf8: EncodedSpan { start: 0, end: 0 },
        utf16: EncodedSpan { start: 0, end: 0 },
        utf32: EncodedSpan { start: 0, end: 0 },
        kind: OriginKind::CoarseZeroWidth,
    }
}

fn coarse_on_line(lines: &[LineSpan], one_based_line: usize) -> EncodedOrigin {
    let line = one_based_line.saturating_sub(1);
    if line < lines.len() {
        EncodedOrigin {
            line: line as u32,
            utf8: EncodedSpan { start: 0, end: 0 },
            utf16: EncodedSpan { start: 0, end: 0 },
            utf32: EncodedSpan { start: 0, end: 0 },
            kind: OriginKind::CoarseZeroWidth,
        }
    } else {
        missing_input_origin()
    }
}

fn exact_origin(
    captured: &CapturedFile<'_>,
    span: ParserByteSpan,
    finding: &Finding,
) -> Option<EncodedOrigin> {
    if finding.probe.expression.is_empty() {
        return None;
    }
    let source = captured.source.as_ref();
    let start = trimmed_expression_start(source, span.start_byte, &finding.probe.expression)?;
    let end = start.checked_add(finding.probe.expression.len())?;
    if end > source.len() || !source.is_char_boundary(start) || !source.is_char_boundary(end) {
        return None;
    }
    if source.get(start..end) != Some(finding.probe.expression.as_str()) {
        return None;
    }
    let (line_index, line_span) = line_containing(start, &captured.lines)?;
    if end > line_span.end {
        return None;
    }
    if finding.probe.location.line.saturating_sub(1) != line_index {
        return None;
    }
    Some(EncodedOrigin {
        line: line_index as u32,
        utf8: captured
            .coords
            .line_span(line_span.start, start, end, Encoding::Utf8)?,
        utf16: captured
            .coords
            .line_span(line_span.start, start, end, Encoding::Utf16)?,
        utf32: captured
            .coords
            .line_span(line_span.start, start, end, Encoding::Utf32)?,
        kind: OriginKind::Exact,
    })
}

fn trimmed_expression_start(source: &str, start_byte: usize, expression: &str) -> Option<usize> {
    let rest = source.get(start_byte..)?;
    let skipped = rest.len() - rest.trim_start().len();
    let exact_start = start_byte.checked_add(skipped)?;
    source.get(exact_start..exact_start.checked_add(expression.len())?)?;
    Some(exact_start)
}

#[derive(Clone, Copy, Debug)]
enum Encoding {
    Utf8,
    Utf16,
    Utf32,
}

#[cfg(test)]
fn encoding_width(text: &str, encoding: Encoding) -> u32 {
    match encoding {
        Encoding::Utf8 => text.len() as u32,
        Encoding::Utf32 => text.chars().count() as u32,
        Encoding::Utf16 => text.chars().map(|ch| ch.len_utf16() as u32).sum(),
    }
}

#[cfg(test)]
fn capped_width(expression: &str, encoding: Encoding) -> u32 {
    let mut width = 0u32;
    for ch in expression.chars() {
        let extra = match encoding {
            Encoding::Utf8 => ch.len_utf8() as u32,
            Encoding::Utf16 => ch.len_utf16() as u32,
            Encoding::Utf32 => 1,
        };
        if width.saturating_add(extra) > ORIGIN_MAX_SPAN_WIDTH {
            break;
        }
        width = width.saturating_add(extra);
    }
    width
}

#[derive(Clone, Copy)]
struct LineSpan {
    start: usize,
    end: usize,
}

fn has_standalone_cr(source: &str) -> bool {
    let bytes = source.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' {
            if bytes.get(index + 1) == Some(&b'\n') {
                index += 2;
            } else {
                return true;
            }
        } else {
            index += 1;
        }
    }
    false
}

fn lsp_lines(source: &str) -> Vec<LineSpan> {
    let bytes = source.as_bytes();
    let mut lines = Vec::new();
    let mut start = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            lines.push(LineSpan { start, end: index });
            index += 2;
            start = index;
        } else if bytes[index] == b'\n' {
            lines.push(LineSpan { start, end: index });
            index += 1;
            start = index;
        } else {
            index += 1;
        }
    }
    if start < bytes.len() || bytes.is_empty() || source.ends_with('\n') || source.ends_with("\r\n")
    {
        lines.push(LineSpan {
            start,
            end: bytes.len(),
        });
    }
    if lines.is_empty() {
        lines.push(LineSpan { start: 0, end: 0 });
    }
    lines
}

fn line_containing(offset: usize, lines: &[LineSpan]) -> Option<(usize, LineSpan)> {
    lines
        .iter()
        .enumerate()
        .find(|(_, line)| offset >= line.start && offset <= line.end)
        .map(|(index, line)| (index, *line))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rust_index::FileFacts;
    use crate::domain::{
        ActivationEvidence, Confidence, DeltaKind, ExposureClass, Probe, ProbeFamily, ProbeId,
        RevealEvidence, RiprEvidence, SourceCurrentness, SourceLocation, StageEvidence, StageState,
    };
    use std::borrow::Cow;
    use std::path::PathBuf;

    const PREDICATE: &str = "montant_é > discount_threshold";

    fn stage() -> StageEvidence {
        StageEvidence::new(StageState::Unknown, Confidence::Unknown, "origin test")
    }

    fn finding_on(
        id: &str,
        relative: &str,
        line: usize,
        expression: &str,
        currentness: SourceCurrentness,
    ) -> Finding {
        Finding {
            id: id.to_string(),
            canonical_gap: None,
            probe: Probe {
                id: ProbeId(id.to_string()),
                location: SourceLocation::new(PathBuf::from(relative), line, 1),
                owner: None,
                family: ProbeFamily::Predicate,
                delta: DeltaKind::Control,
                before: None,
                after: Some(expression.to_string()),
                expression: expression.to_string(),
                expected_sinks: Vec::new(),
                required_oracles: Vec::new(),
            },
            class: ExposureClass::NoStaticPath,
            ripr: RiprEvidence {
                reach: stage(),
                infect: stage(),
                propagate: stage(),
                reveal: RevealEvidence {
                    observe: stage(),
                    discriminate: stage(),
                },
            },
            confidence: 0.0,
            evidence: Vec::new(),
            missing: Vec::new(),
            flow_sinks: Vec::new(),
            activation: ActivationEvidence::default(),
            stop_reasons: Vec::new(),
            related_tests_matched_total: None,
            related_tests: Vec::new(),
            recommended_next_step: None,
            language: Some(crate::domain::LanguageId::Rust),
            language_status: None,
            owner_kind: None,
            static_limit_kind: None,
            changed_sink: None,
            observed_sink: None,
            oracle_alignment: None,
            alignment_reason: None,
            source_currentness: currentness,
        }
    }

    fn facts_for(source: &str) -> FileFacts {
        FileFacts {
            path: PathBuf::from("src/lib.rs"),
            source: source.to_string(),
            ..FileFacts::default()
        }
    }

    fn index_with(source: &str) -> RustIndex {
        let mut index = RustIndex::default();
        index.insert_file_only(PathBuf::from("src/lib.rs"), facts_for(source));
        index
    }

    fn origins_for(
        finding: &Finding,
        source: &str,
        span: Option<ParserByteSpan>,
        facts_source: Option<&str>,
    ) -> RustDiagnosticOrigins {
        let mut index = RustIndex::default();
        if let Some(facts_source) = facts_source {
            index.insert_file_only(PathBuf::from("src/lib.rs"), facts_for(facts_source));
        } else {
            index = index_with(source);
        }
        let mut spans = BTreeMap::new();
        if let Some(span) = span {
            spans.insert(finding.id.clone(), span);
        }
        let loaded = vec![(PathBuf::from("src/lib.rs"), source.as_bytes().to_vec())];
        origins_for_rust_findings(
            std::slice::from_ref(finding),
            &OriginBuildContext {
                root: Path::new("/workspace"),
                loaded_files: &loaded,
                index: &index,
                parser_spans: &spans,
            },
        )
    }

    fn require_find(source: &str, needle: &str, label: &str) -> Result<usize, String> {
        source
            .find(needle)
            .ok_or_else(|| format!("{label} {needle:?} missing from {source:?}"))
    }

    fn require_second(source: &str, needle: &str) -> Result<(usize, usize), String> {
        let first = require_find(source, needle, "first")?;
        let rest = source
            .get(first.saturating_add(1)..)
            .ok_or_else(|| format!("slice after first {needle:?} is not a scalar boundary"))?;
        let second = rest
            .find(needle)
            .map(|offset| first.saturating_add(1).saturating_add(offset))
            .ok_or_else(|| format!("second {needle:?} missing from {source:?}"))?;
        Ok((first, second))
    }

    fn require_origin<'a>(
        origins: &'a RustDiagnosticOrigins,
        id: &str,
    ) -> Result<&'a EncodedOrigin, String> {
        origins
            .get(id)
            .ok_or_else(|| format!("missing origin {id}"))
    }

    fn covered<'a>(source: &'a str, origin: &EncodedOrigin) -> Result<&'a str, String> {
        let line = lsp_lines(source)
            .into_iter()
            .nth(origin.line as usize)
            .ok_or_else(|| format!("missing line {}", origin.line))?;
        let line_text = source
            .get(line.start..line.end)
            .ok_or_else(|| "line bytes are not a scalar slice".to_string())?;
        let start = origin.utf8.start as usize;
        let end = origin.utf8.end as usize;
        line_text
            .get(start..end)
            .ok_or_else(|| format!("covered {start}..{end} splits a scalar"))
    }

    fn current_finding(id: &str, line: usize, expression: &str) -> Finding {
        finding_on(
            id,
            "src/lib.rs",
            line,
            expression,
            SourceCurrentness::CandidateCurrent,
        )
    }

    #[test]
    fn exact_origin_skips_indent_and_if_keyword() -> Result<(), String> {
        let source =
            "fn price() {\n    if montant_é > discount_threshold {\n        true\n    }\n}\n";
        let start = require_find(source, PREDICATE, "predicate")?;
        let finding = current_finding("probe:first", 2, PREDICATE);
        let origins = origins_for(
            &finding,
            source,
            Some(ParserByteSpan { start_byte: start }),
            None,
        );
        let origin = require_origin(&origins, "probe:first")?;
        assert_eq!(origin.kind, OriginKind::Exact);
        assert_eq!(origin.line, 1);
        assert_eq!(covered(source, origin)?, PREDICATE);
        assert_eq!(origin.utf16.start, 7);
        assert_eq!(
            origin.utf16.end,
            origin.utf16.start + encoding_width(PREDICATE, Encoding::Utf16)
        );
        assert_eq!(origin.utf32.start, 7);
        assert_eq!(origin.utf8.start, "    if ".len() as u32);
        Ok(())
    }

    #[test]
    fn first_substring_on_the_line_is_not_the_producer_span() -> Result<(), String> {
        let source = concat!(
            "fn price() {\n",
            "    let _ = \"montant_é > discount_threshold\"; if montant_é > discount_threshold {\n",
            "        true\n",
            "    }\n",
            "}\n"
        );
        let (decoy, producer) = require_second(source, PREDICATE)?;
        if decoy >= producer {
            return Err(format!("decoy {decoy} is not before producer {producer}"));
        }
        let finding = current_finding("probe:second", 2, PREDICATE);
        let origins = origins_for(
            &finding,
            source,
            Some(ParserByteSpan {
                start_byte: producer,
            }),
            None,
        );
        let origin = require_origin(&origins, "probe:second")?;
        assert_eq!(origin.kind, OriginKind::Exact);
        assert_eq!(covered(source, origin)?, PREDICATE);
        let line = source
            .lines()
            .nth(1)
            .ok_or_else(|| "missing predicate line".to_string())?;
        let first_match = require_find(line, PREDICATE, "first match")?;
        assert_ne!(origin.utf8.start as usize, first_match);
        let newline = require_find(source, "\n", "newline")?;
        assert_eq!(origin.utf8.start as usize, producer - (newline + 1));
        Ok(())
    }

    #[test]
    fn cached_facts_from_another_source_cannot_authorize_captured_input() -> Result<(), String> {
        let captured = "fn a() {\n    if montant_é > discount_threshold { true }\n}\n";
        let cached = "fn b() {\n    if montant_é > discount_threshold { true }\n}\n";
        if captured.len() != cached.len() {
            return Err(
                "fixture lengths must match so the parser offset is valid in both".to_string(),
            );
        }
        let start = require_find(captured, PREDICATE, "captured predicate")?;
        let cached_start = require_find(cached, PREDICATE, "cached predicate")?;
        if start != cached_start {
            return Err(format!(
                "predicate offsets must coincide: captured {start} vs cached {cached_start}"
            ));
        }
        if captured == cached {
            return Err("fixtures must differ as whole files".to_string());
        }
        let finding = current_finding("probe:a", 2, PREDICATE);
        let origins = origins_for(
            &finding,
            captured,
            Some(ParserByteSpan { start_byte: start }),
            Some(cached),
        );
        let origin = require_origin(&origins, "probe:a")?;
        assert_eq!(origin.kind, OriginKind::CoarseZeroWidth);
        assert_eq!(origin.utf8.start, 0);
        assert_eq!(origin.utf8.end, 0);
        assert_eq!(origin.utf16.end, 0);
        assert_eq!(origin.utf32.end, 0);
        Ok(())
    }

    #[test]
    fn candidate_current_without_parser_span_stays_coarse() -> Result<(), String> {
        let source = "fn price() {\n    if montant_é > discount_threshold { true }\n}\n";
        let finding = current_finding("probe:line", 2, PREDICATE);
        let origins = origins_for(&finding, source, None, None);
        let origin = require_origin(&origins, "probe:line")?;
        assert_eq!(origin.kind, OriginKind::CoarseZeroWidth);
        assert_eq!(origin.line, 1);
        assert_eq!(origin.utf8.start, 0);
        assert_eq!(origin.utf8.end, 0);
        Ok(())
    }

    #[test]
    fn current_without_span_and_without_loaded_source_records_missing_input() -> Result<(), String>
    {
        let finding = current_finding("probe:missing", 2, PREDICATE);
        let origins = origins_for_rust_findings(
            std::slice::from_ref(&finding),
            &OriginBuildContext {
                root: Path::new("/workspace"),
                loaded_files: &[],
                index: &RustIndex::default(),
                parser_spans: &BTreeMap::new(),
            },
        );
        let origin = require_origin(&origins, "probe:missing")?;
        assert_eq!(origin, &missing_input_origin());
        Ok(())
    }

    #[test]
    fn lexical_fallback_facts_refuse_precision() -> Result<(), String> {
        let source = "fn price() {\n    if montant_é > discount_threshold { true }\n}\n";
        let start = require_find(source, PREDICATE, "predicate")?;
        let finding = current_finding("probe:lex", 2, PREDICATE);
        let mut index = index_with(source);
        if let Some(facts) = index.file_data_mut(&PathBuf::from("src/lib.rs")) {
            facts.used_lexical_fallback = true;
        } else {
            return Err("facts missing".to_string());
        }
        let mut spans = BTreeMap::new();
        spans.insert(finding.id.clone(), ParserByteSpan { start_byte: start });
        let loaded = vec![(PathBuf::from("src/lib.rs"), source.as_bytes().to_vec())];
        let origins = origins_for_rust_findings(
            std::slice::from_ref(&finding),
            &OriginBuildContext {
                root: Path::new("/workspace"),
                loaded_files: &loaded,
                index: &index,
                parser_spans: &spans,
            },
        );
        let origin = require_origin(&origins, "probe:lex")?;
        assert_eq!(origin.kind, OriginKind::CoarseZeroWidth);
        assert_eq!(origin.utf8.end, origin.utf8.start);
        Ok(())
    }

    #[test]
    fn expression_not_at_parser_start_stays_coarse() -> Result<(), String> {
        let source = "fn price() {\n    if montant_é > discount_threshold { true }\n}\n";
        let start = require_find(source, "if ", "if keyword")?;
        let finding = current_finding("probe:mismatch", 2, PREDICATE);
        let origins = origins_for(
            &finding,
            source,
            Some(ParserByteSpan { start_byte: start }),
            None,
        );
        let origin = require_origin(&origins, "probe:mismatch")?;
        assert_eq!(origin.kind, OriginKind::CoarseZeroWidth);
        Ok(())
    }

    #[test]
    fn empty_expression_stays_coarse() -> Result<(), String> {
        let source = "fn price() {\n    if montant_é > discount_threshold { true }\n}\n";
        let start = require_find(source, PREDICATE, "predicate")?;
        let finding = current_finding("probe:empty", 2, "");
        let origins = origins_for(
            &finding,
            source,
            Some(ParserByteSpan { start_byte: start }),
            None,
        );
        let origin = require_origin(&origins, "probe:empty")?;
        assert_eq!(origin.kind, OriginKind::CoarseZeroWidth);
        Ok(())
    }

    #[test]
    fn location_line_mismatch_stays_coarse() -> Result<(), String> {
        let source = "fn price() {\n    if montant_é > discount_threshold { true }\n}\n";
        let start = require_find(source, PREDICATE, "predicate")?;
        let finding = current_finding("probe:line", 1, PREDICATE);
        let origins = origins_for(
            &finding,
            source,
            Some(ParserByteSpan { start_byte: start }),
            None,
        );
        let origin = require_origin(&origins, "probe:line")?;
        assert_eq!(origin.kind, OriginKind::CoarseZeroWidth);
        Ok(())
    }

    #[test]
    fn same_line_span_refuses_a_newline() {
        assert_eq!(ParserByteSpan::same_line("a\nb", 0), None);
        assert_eq!(
            ParserByteSpan::same_line(PREDICATE, 4),
            Some(ParserByteSpan { start_byte: 4 })
        );
    }

    #[test]
    fn base_deleted_is_zero_width_on_the_current_line() -> Result<(), String> {
        let source = "fn price() {\n    true\n}\n";
        let finding = finding_on(
            "probe:deleted",
            "src/lib.rs",
            2,
            "very_long_call(montant_é, discount_threshold, other_argument)",
            SourceCurrentness::BaseDeleted,
        );
        let origins = origins_for(&finding, source, None, None);
        let origin = require_origin(&origins, "probe:deleted")?;
        assert_eq!(origin.kind, OriginKind::CoarseZeroWidth);
        assert_eq!(origin.line, 1);
        assert_eq!(origin.utf8.end, origin.utf8.start);
        assert_eq!(origin.utf16.end, 0);
        assert_eq!(origin.utf32.end, 0);
        Ok(())
    }

    #[test]
    fn standalone_cr_refuses_precision() -> Result<(), String> {
        let source = "fn price() {\r    if montant_é > discount_threshold { true }\r}\r";
        let start = require_find(source, PREDICATE, "predicate")?;
        let finding = current_finding("probe:cr", 2, PREDICATE);
        let origins = origins_for(
            &finding,
            source,
            Some(ParserByteSpan { start_byte: start }),
            None,
        );
        let origin = require_origin(&origins, "probe:cr")?;
        assert_eq!(origin.kind, OriginKind::CoarseZeroWidth);
        assert_eq!(origin.line, 0);
        Ok(())
    }

    #[test]
    fn crlf_admits_exact_geometry() -> Result<(), String> {
        let source = "fn price() {\r\n    if montant_é > discount_threshold {\r\n        true\r\n    }\r\n}\r\n";
        let start = require_find(source, PREDICATE, "predicate")?;
        let finding = current_finding("probe:crlf", 2, PREDICATE);
        let origins = origins_for(
            &finding,
            source,
            Some(ParserByteSpan { start_byte: start }),
            None,
        );
        let origin = require_origin(&origins, "probe:crlf")?;
        assert_eq!(origin.kind, OriginKind::Exact);
        assert_eq!(origin.line, 1);
        assert_eq!(covered(source, origin)?, PREDICATE);
        Ok(())
    }

    #[test]
    fn mid_scalar_start_stays_coarse() -> Result<(), String> {
        let source = "fn price() {\n    if montant_é > discount_threshold { true }\n}\n";
        let accent = require_find(source, "é", "accent")?;
        let start = accent.saturating_add(1);
        if source.is_char_boundary(start) {
            return Err(format!("expected mid-scalar offset, got {start}"));
        }
        let finding = current_finding("probe:mid", 2, PREDICATE);
        let origins = origins_for(
            &finding,
            source,
            Some(ParserByteSpan { start_byte: start }),
            None,
        );
        let origin = require_origin(&origins, "probe:mid")?;
        assert_eq!(origin.kind, OriginKind::CoarseZeroWidth);
        Ok(())
    }

    #[test]
    fn tab_cjk_astral_and_combining_prefixes_count_all_encodings() -> Result<(), String> {
        let prefix = "\tlet 日本語🎉e\u{0301} = 1; if ";
        let source = format!("fn price() {{\n{prefix}{PREDICATE} {{\n        true\n    }}\n}}\n");
        let start = require_find(&source, PREDICATE, "predicate")?;
        let finding = current_finding("probe:prefix", 2, PREDICATE);
        let origins = origins_for(
            &finding,
            &source,
            Some(ParserByteSpan { start_byte: start }),
            None,
        );
        let origin = require_origin(&origins, "probe:prefix")?;
        assert_eq!(origin.kind, OriginKind::Exact);
        assert_eq!(covered(&source, origin)?, PREDICATE);
        assert_eq!(origin.utf8.start, encoding_width(prefix, Encoding::Utf8));
        assert_eq!(origin.utf16.start, encoding_width(prefix, Encoding::Utf16));
        assert_eq!(origin.utf32.start, encoding_width(prefix, Encoding::Utf32));
        assert_ne!(origin.utf8.start, origin.utf16.start);
        assert_ne!(origin.utf16.start, origin.utf32.start);
        Ok(())
    }

    #[test]
    fn missing_loaded_source_uses_line_zero() -> Result<(), String> {
        let finding = current_finding("probe:missing", 2, PREDICATE);
        let mut spans = BTreeMap::new();
        spans.insert(finding.id.clone(), ParserByteSpan { start_byte: 4 });
        let index = RustIndex::default();
        let origins = origins_for_rust_findings(
            std::slice::from_ref(&finding),
            &OriginBuildContext {
                root: Path::new("/workspace"),
                loaded_files: &[],
                index: &index,
                parser_spans: &spans,
            },
        );
        let origin = require_origin(&origins, "probe:missing")?;
        assert_eq!(origin, &missing_input_origin());
        Ok(())
    }

    #[test]
    fn scalar_cap_does_not_split_an_astral_character() -> Result<(), String> {
        let expression = format!("{}{}", "a".repeat(119), "🎉");
        let width = capped_width(&expression, Encoding::Utf16);
        assert_eq!(width, 119);
        let utf8 = capped_width(&expression, Encoding::Utf8);
        assert_eq!(utf8, 119);
        let coords = ScalarIndex::build(&expression);
        let utf16_end = coords
            .capped_end(0, expression.len(), Encoding::Utf16)
            .ok_or_else(|| "astral expression leaves UTF-16 scalar boundaries".to_string())?;
        assert_eq!(utf16_end, 119);
        let utf8_end = coords
            .capped_end(0, expression.len(), Encoding::Utf8)
            .ok_or_else(|| "astral expression leaves UTF-8 scalar boundaries".to_string())?;
        assert_eq!(utf8_end, 119);
        Ok(())
    }

    #[test]
    fn captured_index_borrows_ordinary_utf8_including_bom() -> Result<(), String> {
        let ordinary = b"fn a() { true }\n".to_vec();
        let mut bom = b"\xEF\xBB\xBF".to_vec();
        bom.extend_from_slice(&ordinary);
        let loaded = vec![
            (PathBuf::from("src/lib.rs"), ordinary),
            (PathBuf::from("src/bom.rs"), bom),
        ];
        let captured = captured_file_index(&loaded, &RustIndex::default());
        let ordinary_file = captured
            .get(Path::new("src/lib.rs"))
            .ok_or_else(|| "ordinary file missing".to_string())?;
        let bom_file = captured
            .get(Path::new("src/bom.rs"))
            .ok_or_else(|| "bom file missing".to_string())?;
        if !matches!(ordinary_file.source, Cow::Borrowed(_)) {
            return Err("ordinary valid UTF-8 must stay borrowed".to_string());
        }
        if !matches!(bom_file.source, Cow::Borrowed(_)) {
            return Err("BOM-stripped valid UTF-8 must stay borrowed".to_string());
        }
        assert_eq!(ordinary_file.source.as_ref(), bom_file.source.as_ref());
        Ok(())
    }

    #[test]
    fn captured_index_owns_lossy_non_utf8() -> Result<(), String> {
        let loaded = vec![(PathBuf::from("src/lib.rs"), vec![0xff, 0xfe, b'x'])];
        let captured = captured_file_index(&loaded, &RustIndex::default());
        let file = captured
            .get(Path::new("src/lib.rs"))
            .ok_or_else(|| "lossy file missing".to_string())?;
        if !matches!(file.source, Cow::Owned(_)) {
            return Err("invalid UTF-8 must own the lossy decode".to_string());
        }
        Ok(())
    }

    #[test]
    fn two_predicates_on_one_unicode_line_share_file_coordinates() -> Result<(), String> {
        const SECOND: &str = "montant_é < other_threshold";
        let prefix_a = "\tlet 日本語🎉e\u{0301} = 1; if ";
        let between = " { true } else if ";
        let source = format!(
            "fn price() {{\n{prefix_a}{PREDICATE}{between}{SECOND} {{\n        true\n    }}\n}}\n"
        );
        let start_a = require_find(&source, PREDICATE, "first predicate")?;
        let start_b = require_find(&source, SECOND, "second predicate")?;
        let finding_a = current_finding("probe:a", 2, PREDICATE);
        let finding_b = current_finding("probe:b", 2, SECOND);
        let mut spans = BTreeMap::new();
        spans.insert(
            finding_a.id.clone(),
            ParserByteSpan {
                start_byte: start_a,
            },
        );
        spans.insert(
            finding_b.id.clone(),
            ParserByteSpan {
                start_byte: start_b,
            },
        );
        let loaded = vec![(PathBuf::from("src/lib.rs"), source.as_bytes().to_vec())];
        let captured = captured_file_index(&loaded, &index_with(&source));
        let file = captured
            .get(Path::new("src/lib.rs"))
            .ok_or_else(|| "captured file missing".to_string())?;
        if !matches!(file.source, Cow::Borrowed(_)) {
            return Err("fixture UTF-8 must stay borrowed".to_string());
        }
        let origins = origins_for_rust_findings(
            &[finding_a, finding_b],
            &OriginBuildContext {
                root: Path::new("/workspace"),
                loaded_files: &loaded,
                index: &index_with(&source),
                parser_spans: &spans,
            },
        );
        let origin_a = require_origin(&origins, "probe:a")?;
        let origin_b = require_origin(&origins, "probe:b")?;
        assert_eq!(origin_a.kind, OriginKind::Exact);
        assert_eq!(origin_b.kind, OriginKind::Exact);
        assert_eq!(origin_a.line, origin_b.line);
        let line_start = file
            .lines
            .get(origin_a.line as usize)
            .ok_or_else(|| "missing predicate line".to_string())?
            .start;
        for encoding in [Encoding::Utf8, Encoding::Utf16, Encoding::Utf32] {
            let start_a_units = file
                .coords
                .line_span(line_start, start_a, start_a + PREDICATE.len(), encoding)
                .ok_or_else(|| format!("{encoding:?} missing first span"))?;
            let start_b_units = file
                .coords
                .line_span(line_start, start_b, start_b + SECOND.len(), encoding)
                .ok_or_else(|| format!("{encoding:?} missing second span"))?;
            let origin_span = match encoding {
                Encoding::Utf8 => origin_a.utf8,
                Encoding::Utf16 => origin_a.utf16,
                Encoding::Utf32 => origin_a.utf32,
            };
            assert_eq!(origin_span, start_a_units);
            let origin_b_span = match encoding {
                Encoding::Utf8 => origin_b.utf8,
                Encoding::Utf16 => origin_b.utf16,
                Encoding::Utf32 => origin_b.utf32,
            };
            assert_eq!(origin_b_span, start_b_units);
        }
        let prefix_b = format!("{prefix_a}{PREDICATE}{between}");
        assert_eq!(
            origin_a.utf8.start,
            encoding_width(prefix_a, Encoding::Utf8)
        );
        assert_eq!(
            origin_a.utf16.start,
            encoding_width(prefix_a, Encoding::Utf16)
        );
        assert_eq!(
            origin_a.utf32.start,
            encoding_width(prefix_a, Encoding::Utf32)
        );
        assert_eq!(
            origin_b.utf8.start,
            encoding_width(&prefix_b, Encoding::Utf8)
        );
        assert_eq!(
            origin_b.utf16.start,
            encoding_width(&prefix_b, Encoding::Utf16)
        );
        assert_eq!(
            origin_b.utf32.start,
            encoding_width(&prefix_b, Encoding::Utf32)
        );
        assert_ne!(origin_a.utf8.start, origin_a.utf16.start);
        assert_ne!(origin_a.utf16.start, origin_a.utf32.start);
        assert_ne!(origin_a.utf8.start, origin_b.utf8.start);
        Ok(())
    }
}
