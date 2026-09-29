//! Producer-path origin tests for #4464.
//!
//! These go through the real Rust diff/repo producers and the standard
//! diagnostic construction path. Unit arithmetic in `diagnostic_origin` is
//! not sufficient on its own.

use super::config::LspAnalysisConfig;
use super::diagnostics::{
    FindingDiagnosticProjection, finding_diagnostics_by_uri_with_profile,
    workspace_diagnostics_with_config,
};
use super::position::expression_span_range_on_saved_line;
use super::tests::{run_lsp_scope_git, unique_lsp_test_root};
use crate::analysis::diagnostic_origin::{
    EncodedOrigin, EncodedSpan, OriginKind, RustDiagnosticOrigins,
};
use crate::app::{CheckInput, Mode, check_workspace_repo_with_origins};
use crate::config::LspDiagnosticProfile;
use crate::domain::SourceCurrentness;
use std::fs;
use std::path::Path;
use tower_lsp_server::ls_types::{Diagnostic, PositionEncodingKind};

const PREDICATE: &str = "montant_é > discount_threshold";

const DECOY_LINE: &str = "    let decoy = \"montant_é > discount_threshold { false } else { true }\"; if montant_é > discount_threshold { true } else { false }";

const SECOND_LINE: &str = "    if montant_é > discount_threshold { false } else { true }";

fn write_origin_lib(root: &Path, extra_lines: &str) -> Result<(), String> {
    fs::create_dir_all(root.join("src")).map_err(|err| format!("create src: {err}"))?;
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"origin-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write Cargo.toml: {err}"))?;
    let body = format!(
        "pub fn price(montant_é: i32, discount_threshold: i32) -> bool {{\n{DECOY_LINE}\n{SECOND_LINE}\n{extra_lines}}}\n"
    );
    fs::write(root.join("src/lib.rs"), body).map_err(|err| format!("write lib.rs: {err}"))
}

fn init_git_repo(root: &Path) -> Result<(), String> {
    run_lsp_scope_git(root, &["init"])?;
    run_lsp_scope_git(root, &["config", "user.email", "ripr@example.invalid"])?;
    run_lsp_scope_git(root, &["config", "user.name", "RIPR Test"])?;
    run_lsp_scope_git(root, &["add", "Cargo.toml", "src/lib.rs"])?;
    run_lsp_scope_git(root, &["commit", "-m", "base"])?;
    Ok(())
}

fn instant_full_config(encoding: PositionEncodingKind) -> LspAnalysisConfig {
    LspAnalysisConfig {
        base_ref: Some("HEAD".to_string()),
        mode: Mode::Instant,
        diagnostic_profile: LspDiagnosticProfile::Full,
        position_encoding: encoding,
        ..LspAnalysisConfig::default()
    }
}

fn covered_text(
    line: &str,
    start: u32,
    end: u32,
    encoding: &PositionEncodingKind,
) -> Result<String, String> {
    if start > end {
        return Err(format!("reversed range {start}..{end}"));
    }
    let mut units = 0u32;
    let mut start_byte = None;
    let mut end_byte = None;
    for (offset, ch) in line.char_indices() {
        if units == start {
            start_byte = Some(offset);
        }
        if units == end {
            end_byte = Some(offset);
            break;
        }
        let width = if *encoding == PositionEncodingKind::UTF8 {
            ch.len_utf8() as u32
        } else if *encoding == PositionEncodingKind::UTF32 {
            1
        } else {
            ch.len_utf16() as u32
        };
        units = units.saturating_add(width);
    }
    if units == start {
        start_byte = Some(line.len());
    }
    if units == end {
        end_byte = Some(line.len());
    }
    let start_byte = start_byte.ok_or_else(|| format!("start {start} is not a scalar boundary"))?;
    let end_byte = end_byte.ok_or_else(|| format!("end {end} is not a scalar boundary"))?;
    if !line.is_char_boundary(start_byte) || !line.is_char_boundary(end_byte) {
        return Err("range splits a scalar".to_string());
    }
    Ok(line[start_byte..end_byte].to_string())
}

fn predicate_diagnostics<'a>(
    diagnostics: &'a [Diagnostic],
    source: &str,
) -> Result<Vec<&'a Diagnostic>, String> {
    let selected: Vec<&Diagnostic> = diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("probe_family"))
                .and_then(|value| value.as_str())
                == Some("predicate")
        })
        .collect();
    if selected.is_empty() {
        return Err(format!(
            "no predicate diagnostics from source:\n{source}\nall={diagnostics:?}"
        ));
    }
    Ok(selected)
}

fn encoding_width(text: &str, encoding: &PositionEncodingKind) -> u32 {
    if *encoding == PositionEncodingKind::UTF8 {
        text.len() as u32
    } else if *encoding == PositionEncodingKind::UTF32 {
        text.chars().count() as u32
    } else {
        text.chars().map(|ch| ch.len_utf16() as u32).sum()
    }
}

fn source_line(source: &str, lsp_line: u32) -> Result<&str, String> {
    source
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .nth(lsp_line as usize)
        .ok_or_else(|| format!("missing line {lsp_line} in {source:?}"))
}

fn producer_predicate_start(
    line_text: &str,
    encoding: &PositionEncodingKind,
) -> Result<u32, String> {
    let decoy = line_text
        .find(PREDICATE)
        .ok_or_else(|| format!("predicate missing from {line_text:?}"))?;
    let producer = line_text[decoy + 1..]
        .find(PREDICATE)
        .map(|offset| decoy + 1 + offset)
        .unwrap_or(decoy);
    if !line_text.is_char_boundary(producer) {
        return Err(format!("producer start splits a scalar in {line_text:?}"));
    }
    Ok(encoding_width(&line_text[..producer], encoding))
}

fn assert_covers_producer_predicate(
    source: &str,
    diagnostic: &Diagnostic,
    encoding: &PositionEncodingKind,
    reject_first_match: bool,
) -> Result<(), String> {
    let line_text = source_line(source, diagnostic.range.start.line)?;
    let covered = covered_text(
        line_text,
        diagnostic.range.start.character,
        diagnostic.range.end.character,
        encoding,
    )?;
    if !covered.contains(PREDICATE) {
        return Err(format!(
            "covered {covered:?} does not contain {PREDICATE:?}"
        ));
    }
    let producer_start = producer_predicate_start(line_text, encoding)?;
    if diagnostic.range.start.character != producer_start {
        return Err(format!(
            "start {} is not the producer predicate at {producer_start} on {line_text:?}",
            diagnostic.range.start.character
        ));
    }
    if reject_first_match {
        let heuristic = expression_span_range_on_saved_line(
            diagnostic.range.start.line,
            1,
            PREDICATE,
            encoding,
            Some(line_text),
        );
        if diagnostic.range.start.character == heuristic.start.character {
            return Err(
                "first-substring heuristic selected the string decoy on the saved line".to_string(),
            );
        }
    }
    Ok(())
}

fn flip_decoy_if(source: &str) -> String {
    source.replace(
        "if montant_é > discount_threshold { true } else { false }",
        "if montant_é > discount_threshold { false } else { true }",
    )
}

#[test]
fn worktree_diff_selects_producer_occurrence_not_first_match() -> Result<(), String> {
    let root = unique_lsp_test_root("origin-diff-decoy")?;
    write_origin_lib(root.path(), "")?;
    init_git_repo(root.path())?;
    let committed =
        fs::read_to_string(root.path().join("src/lib.rs")).map_err(|err| format!("read: {err}"))?;
    let source = flip_decoy_if(&committed);
    fs::write(root.path().join("src/lib.rs"), &source).map_err(|err| format!("edit: {err}"))?;

    let encodings = [
        PositionEncodingKind::UTF8,
        PositionEncodingKind::UTF16,
        PositionEncodingKind::UTF32,
    ];
    let mut seen_ids = Vec::new();
    for encoding in encodings {
        let diagnostics = workspace_diagnostics_with_config(
            root.path(),
            &instant_full_config(encoding.clone()),
            true,
        )?;
        if diagnostics.snapshot.findings.is_empty() {
            return Err("diff producer emitted no findings".to_string());
        }
        let batch = diagnostics
            .batches
            .iter()
            .find(|batch| !batch.diagnostics.is_empty())
            .ok_or_else(|| "no diagnostic batch".to_string())?;
        let selected = predicate_diagnostics(&batch.diagnostics, &source)?;
        let mut ids = Vec::new();
        let mut saw_decoy_line = false;
        for diagnostic in &selected {
            let reject_first_match = diagnostic.range.start.line == 1;
            if reject_first_match {
                saw_decoy_line = true;
            }
            assert_covers_producer_predicate(&source, diagnostic, &encoding, reject_first_match)?;
            let id = diagnostic
                .data
                .as_ref()
                .and_then(|data| data.get("finding_id"))
                .and_then(|value| value.as_str())
                .ok_or_else(|| "missing finding_id".to_string())?;
            ids.push(id.to_string());
        }
        if !saw_decoy_line {
            return Err("no predicate diagnostic landed on the decoy line".to_string());
        }
        ids.sort();
        ids.dedup();
        if ids.is_empty() {
            return Err("expected predicate finding ids".to_string());
        }
        if seen_ids.is_empty() {
            seen_ids = ids;
        } else if seen_ids != ids {
            return Err(format!(
                "encoding {encoding:?} changed finding ids: {seen_ids:?} vs {ids:?}"
            ));
        }
    }
    Ok(())
}

#[test]
fn repo_producer_projects_the_same_predicate_span() -> Result<(), String> {
    let root = unique_lsp_test_root("origin-repo")?;
    write_origin_lib(root.path(), "")?;
    let source = fs::read_to_string(root.path().join("src/lib.rs"))
        .map_err(|err| format!("read lib.rs: {err}"))?;
    let input = CheckInput {
        root: root.path().to_path_buf(),
        mode: Mode::Instant,
        ..CheckInput::default()
    };
    let (output, origins) =
        check_workspace_repo_with_origins(input, &crate::config::RiprConfig::default())?;
    if output.findings.is_empty() {
        return Err("repo producer emitted no findings".to_string());
    }
    if origins.is_empty() {
        return Err("repo producer stored no diagnostic origins".to_string());
    }
    let grouped = finding_diagnostics_by_uri_with_profile(
        root.path(),
        &output.findings,
        &crate::config::SeverityConfig::default(),
        true,
        FindingDiagnosticProjection::new(
            LspDiagnosticProfile::Full,
            &PositionEncodingKind::UTF16,
            &origins,
        ),
    )?;
    let diagnostics: Vec<Diagnostic> = grouped.into_values().flatten().collect();
    let selected = predicate_diagnostics(&diagnostics, &source)?;
    let mut saw_decoy_line = false;
    for diagnostic in selected {
        let reject_first_match = diagnostic.range.start.line == 1;
        if reject_first_match {
            saw_decoy_line = true;
        }
        assert_covers_producer_predicate(
            &source,
            diagnostic,
            &PositionEncodingKind::UTF16,
            reject_first_match,
        )?;
    }
    if !saw_decoy_line {
        return Err("repo producer did not project the decoy-line predicate".to_string());
    }
    Ok(())
}

#[test]
fn tab_cjk_astral_combining_prefix_covers_the_predicate() -> Result<(), String> {
    let root = unique_lsp_test_root("origin-prefix")?;
    fs::create_dir_all(root.path().join("src")).map_err(|err| format!("create src: {err}"))?;
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"origin-prefix\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write Cargo.toml: {err}"))?;
    let prefix = "\tlet 日本語 = \"🎉e\u{0301}\"; if ";
    let source = format!(
        "pub fn price(montant_é: i32, discount_threshold: i32) -> bool {{\n{prefix}{PREDICATE} {{ true }} else {{ false }}\n    true\n}}\n"
    );
    fs::write(root.path().join("src/lib.rs"), &source)
        .map_err(|err| format!("write lib: {err}"))?;
    init_git_repo(root.path())?;
    let edited = flip_decoy_if(&source);
    fs::write(root.path().join("src/lib.rs"), &edited).map_err(|err| format!("edit: {err}"))?;
    for encoding in [
        PositionEncodingKind::UTF8,
        PositionEncodingKind::UTF16,
        PositionEncodingKind::UTF32,
    ] {
        let diagnostics = workspace_diagnostics_with_config(
            root.path(),
            &instant_full_config(encoding.clone()),
            true,
        )?;
        let batch = diagnostics
            .batches
            .iter()
            .find(|batch| !batch.diagnostics.is_empty())
            .ok_or_else(|| "no diagnostic batch".to_string())?;
        let selected = predicate_diagnostics(&batch.diagnostics, &edited)?;
        let mut matched = false;
        for diagnostic in selected {
            if diagnostic.range.start.line != 1 {
                continue;
            }
            assert_covers_producer_predicate(&edited, diagnostic, &encoding, false)?;
            if diagnostic.range.start.character != encoding_width(prefix, &encoding) {
                return Err(format!(
                    "{encoding:?} start {} != prefix width {}",
                    diagnostic.range.start.character,
                    encoding_width(prefix, &encoding)
                ));
            }
            matched = true;
        }
        if !matched {
            return Err(format!("{encoding:?} did not cover the intended predicate"));
        }
    }
    Ok(())
}

#[test]
fn crlf_file_covers_the_predicate() -> Result<(), String> {
    let root = unique_lsp_test_root("origin-crlf")?;
    fs::create_dir_all(root.path().join("src")).map_err(|err| format!("create src: {err}"))?;
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"origin-crlf\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write Cargo.toml: {err}"))?;
    let source = "pub fn price(montant_é: i32, discount_threshold: i32) -> bool {\r\n    if montant_é > discount_threshold { true } else { false }\r\n}\r\n";
    fs::write(root.path().join("src/lib.rs"), source).map_err(|err| format!("write: {err}"))?;
    init_git_repo(root.path())?;
    let edited = flip_decoy_if(source);
    fs::write(root.path().join("src/lib.rs"), &edited).map_err(|err| format!("edit: {err}"))?;
    let diagnostics = workspace_diagnostics_with_config(
        root.path(),
        &instant_full_config(PositionEncodingKind::UTF16),
        true,
    )?;
    let batch = diagnostics
        .batches
        .iter()
        .find(|batch| !batch.diagnostics.is_empty())
        .ok_or_else(|| "no diagnostic batch".to_string())?;
    let selected = predicate_diagnostics(&batch.diagnostics, &edited)?;
    let diagnostic = selected
        .first()
        .ok_or_else(|| "predicate diagnostic missing".to_string())?;
    assert_covers_producer_predicate(&edited, diagnostic, &PositionEncodingKind::UTF16, false)?;
    Ok(())
}

#[test]
fn base_deleted_finding_is_zero_width_on_the_current_line() -> Result<(), String> {
    let root = unique_lsp_test_root("origin-deleted")?;
    fs::create_dir_all(root.path().join("src")).map_err(|err| format!("create src: {err}"))?;
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"origin-deleted\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .map_err(|err| format!("write Cargo.toml: {err}"))?;
    fs::write(
        root.path().join("src/lib.rs"),
        "pub fn price() -> i32 {\n    very_long_call(montant_é, discount_threshold, other_argument)\n}\n",
    )
    .map_err(|err| format!("write: {err}"))?;
    init_git_repo(root.path())?;
    let current = "pub fn price() -> i32 {\n    0\n}\n";
    fs::write(root.path().join("src/lib.rs"), current).map_err(|err| format!("edit: {err}"))?;
    let diagnostics = workspace_diagnostics_with_config(
        root.path(),
        &instant_full_config(PositionEncodingKind::UTF16),
        true,
    )?;
    let deleted: Vec<_> = diagnostics
        .snapshot
        .findings
        .iter()
        .filter(|finding| finding.source_currentness == SourceCurrentness::BaseDeleted)
        .collect();
    if deleted.is_empty() {
        return Err(format!(
            "expected a BaseDeleted finding, got {:?}",
            diagnostics
                .snapshot
                .findings
                .iter()
                .map(|finding| finding.source_currentness)
                .collect::<Vec<_>>()
        ));
    }
    let ids: Vec<&str> = deleted.iter().map(|finding| finding.id.as_str()).collect();
    let batch = diagnostics
        .batches
        .iter()
        .find(|batch| !batch.diagnostics.is_empty())
        .ok_or_else(|| "no diagnostic batch".to_string())?;
    let mut saw_zero_width = false;
    for diagnostic in &batch.diagnostics {
        let Some(id) = diagnostic
            .data
            .as_ref()
            .and_then(|data| data.get("finding_id"))
            .and_then(|value| value.as_str())
        else {
            continue;
        };
        if !ids.contains(&id) {
            continue;
        }
        if diagnostic.range.start.character != diagnostic.range.end.character {
            return Err(format!(
                "BaseDeleted diagnostic must be zero-width, got {:?}",
                diagnostic.range
            ));
        }
        let line_text = current
            .lines()
            .nth(diagnostic.range.start.line as usize)
            .ok_or_else(|| "deleted diagnostic line missing from current source".to_string())?;
        let _ = covered_text(
            line_text,
            diagnostic.range.start.character,
            diagnostic.range.end.character,
            &PositionEncodingKind::UTF16,
        )?;
        saw_zero_width = true;
    }
    if !saw_zero_width {
        return Err("BaseDeleted finding did not reach a diagnostic".to_string());
    }
    Ok(())
}

#[test]
fn cold_and_warm_file_facts_preserve_origin_geometry() -> Result<(), String> {
    let root = unique_lsp_test_root("origin-cache")?;
    write_origin_lib(root.path(), "")?;
    init_git_repo(root.path())?;
    let source =
        fs::read_to_string(root.path().join("src/lib.rs")).map_err(|err| format!("read: {err}"))?;
    let edited = flip_decoy_if(&source);
    fs::write(root.path().join("src/lib.rs"), &edited).map_err(|err| format!("edit: {err}"))?;
    let encoding = PositionEncodingKind::UTF16;
    let cold_diagnostics = workspace_diagnostics_with_config(
        root.path(),
        &instant_full_config(encoding.clone()),
        true,
    )?;
    let warm_diagnostics = workspace_diagnostics_with_config(
        root.path(),
        &instant_full_config(encoding.clone()),
        true,
    )?;
    let cold_ranges: Vec<_> = cold_diagnostics
        .batches
        .iter()
        .flat_map(|batch| batch.diagnostics.iter().map(|diagnostic| diagnostic.range))
        .collect();
    let warm_ranges: Vec<_> = warm_diagnostics
        .batches
        .iter()
        .flat_map(|batch| batch.diagnostics.iter().map(|diagnostic| diagnostic.range))
        .collect();
    if cold_ranges.is_empty() {
        return Err("cold diagnostics were empty".to_string());
    }
    if cold_ranges != warm_ranges {
        return Err(format!(
            "cold/warm ranges diverged: {cold_ranges:?} vs {warm_ranges:?}"
        ));
    }
    let selected = predicate_diagnostics(
        &cold_diagnostics
            .batches
            .iter()
            .find(|batch| !batch.diagnostics.is_empty())
            .ok_or_else(|| "no diagnostic batch".to_string())?
            .diagnostics,
        &edited,
    )?;
    let mut saw_decoy_line = false;
    for diagnostic in selected {
        let reject_first_match = diagnostic.range.start.line == 1;
        if reject_first_match {
            saw_decoy_line = true;
        }
        assert_covers_producer_predicate(&edited, diagnostic, &encoding, reject_first_match)?;
    }
    if !saw_decoy_line {
        return Err("cached origin fell back to the first-substring decoy".to_string());
    }
    Ok(())
}

#[test]
fn retained_origins_do_not_reread_disk_after_source_changes() -> Result<(), String> {
    let root = unique_lsp_test_root("origin-retained-disk")?;
    write_origin_lib(root.path(), "")?;
    let source_a = fs::read_to_string(root.path().join("src/lib.rs"))
        .map_err(|err| format!("read A: {err}"))?;
    let input = CheckInput {
        root: root.path().to_path_buf(),
        mode: Mode::Instant,
        ..CheckInput::default()
    };
    let (output, origins) =
        check_workspace_repo_with_origins(input, &crate::config::RiprConfig::default())?;
    if output.findings.is_empty() {
        return Err("repo producer emitted no findings".to_string());
    }
    if origins.is_empty() {
        return Err("repo producer stored no diagnostic origins".to_string());
    }
    let encoding = PositionEncodingKind::UTF16;
    let projected_a = finding_diagnostics_by_uri_with_profile(
        root.path(),
        &output.findings,
        &crate::config::SeverityConfig::default(),
        true,
        FindingDiagnosticProjection::new(LspDiagnosticProfile::Full, &encoding, &origins),
    )?;
    let ranges_a: Vec<_> = projected_a
        .values()
        .flatten()
        .map(|diagnostic| diagnostic.range)
        .collect();
    if ranges_a.is_empty() {
        return Err("producer-A projection was empty".to_string());
    }
    let selected_a: Vec<_> = projected_a.values().flatten().cloned().collect();
    let predicates_a = predicate_diagnostics(&selected_a, &source_a)?;
    let mut saw_decoy_line = false;
    for diagnostic in predicates_a {
        let reject_first_match = diagnostic.range.start.line == 1;
        if reject_first_match {
            saw_decoy_line = true;
        }
        assert_covers_producer_predicate(&source_a, diagnostic, &encoding, reject_first_match)?;
    }
    if !saw_decoy_line {
        return Err("producer-A did not project the decoy-line predicate".to_string());
    }

    let source_b = "pub fn price(montant_é: i32, discount_threshold: i32) -> bool {\n    if montant_é > discount_threshold { true } else { false }\n}\n";
    fs::write(root.path().join("src/lib.rs"), source_b).map_err(|err| format!("write B: {err}"))?;
    let projected_b = finding_diagnostics_by_uri_with_profile(
        root.path(),
        &output.findings,
        &crate::config::SeverityConfig::default(),
        true,
        FindingDiagnosticProjection::new(LspDiagnosticProfile::Full, &encoding, &origins),
    )?;
    let ranges_b: Vec<_> = projected_b
        .values()
        .flatten()
        .map(|diagnostic| diagnostic.range)
        .collect();
    if ranges_a != ranges_b {
        return Err(format!(
            "retained origins reread disk B: {ranges_a:?} vs {ranges_b:?}"
        ));
    }

    let heuristic_b = finding_diagnostics_by_uri_with_profile(
        root.path(),
        &output.findings,
        &crate::config::SeverityConfig::default(),
        true,
        FindingDiagnosticProjection::new(
            LspDiagnosticProfile::Full,
            &encoding,
            &RustDiagnosticOrigins::default(),
        ),
    )?;
    let heuristic_ranges: Vec<_> = heuristic_b
        .values()
        .flatten()
        .map(|diagnostic| diagnostic.range)
        .collect();
    if heuristic_ranges == ranges_a {
        return Err(
            "empty-origin heuristic on disk B matched retained A ranges; fixture is not a discriminator"
                .to_string(),
        );
    }
    Ok(())
}

#[test]
fn stored_coarse_origin_is_not_replaced_by_saved_line_search() -> Result<(), String> {
    let root = unique_lsp_test_root("origin-coarse-record")?;
    write_origin_lib(root.path(), "")?;
    let source =
        fs::read_to_string(root.path().join("src/lib.rs")).map_err(|err| format!("read: {err}"))?;
    let input = CheckInput {
        root: root.path().to_path_buf(),
        mode: Mode::Instant,
        ..CheckInput::default()
    };
    let (output, _) =
        check_workspace_repo_with_origins(input, &crate::config::RiprConfig::default())?;
    let Some(finding) = output.findings.iter().find(|finding| {
        finding.probe.expression.contains(PREDICATE) && finding.probe.location.line == 2
    }) else {
        return Err(format!(
            "no decoy-line predicate finding in {:?}",
            output
                .findings
                .iter()
                .map(|finding| (
                    finding.probe.location.line,
                    finding.probe.expression.clone()
                ))
                .collect::<Vec<_>>()
        ));
    };
    let mut coarse = RustDiagnosticOrigins::default();
    let line = finding.probe.location.line.saturating_sub(1) as u32;
    coarse.insert(
        finding.id.clone(),
        EncodedOrigin {
            line,
            utf8: EncodedSpan { start: 0, end: 0 },
            utf16: EncodedSpan { start: 0, end: 0 },
            utf32: EncodedSpan { start: 0, end: 0 },
            kind: OriginKind::CoarseZeroWidth,
        },
    );
    let encoding = PositionEncodingKind::UTF16;
    let grouped = finding_diagnostics_by_uri_with_profile(
        root.path(),
        std::slice::from_ref(finding),
        &crate::config::SeverityConfig::default(),
        true,
        FindingDiagnosticProjection::new(LspDiagnosticProfile::Full, &encoding, &coarse),
    )?;
    let diagnostics: Vec<Diagnostic> = grouped.into_values().flatten().collect();
    let diagnostic = diagnostics
        .first()
        .ok_or_else(|| "coarse origin did not project".to_string())?;
    if diagnostic.range.start.line != line
        || diagnostic.range.start.character != 0
        || diagnostic.range.end.character != 0
    {
        return Err(format!(
            "coarse origin was replaced by a saved-line search on {source:?}: {:?}",
            diagnostic.range
        ));
    }
    let heuristic = expression_span_range_on_saved_line(
        line,
        1,
        PREDICATE,
        &encoding,
        source.lines().nth(line as usize),
    );
    if heuristic.start.character == 0 && heuristic.end.character == 0 {
        return Err("heuristic fixture is already zero-width; not a discriminator".to_string());
    }
    Ok(())
}
