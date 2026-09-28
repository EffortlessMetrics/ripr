//! #1453: identical text is a parameter declaration or a field initializer
//! depending on parser-owned context. Do not erase the parameter change or
//! credit it as exposed; retain an explicit unknown rather than a false field.

use super::classify::parser_probe_shapes_for_changed_line;
use super::diff::probes_for_file;
use crate::analysis::diff::{ChangedFile, ChangedLine};
use crate::analysis::extract::PROBE_SHAPE_UNSAFE_BOUNDARY;
use crate::analysis::rust_index::RustIndex;
use crate::analysis::syntax::{RaRustSyntaxAdapter, RustSyntaxAdapter};
use crate::domain::{Probe, ProbeFamily};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SOURCE: &str = "struct Path;\nstruct Envelope<'a> { out: &'a Path }\nfn project(\n    out: &Path,\n) -> Envelope<'_> {\n    Envelope {\n        out: &Path,\n    }\n}\n";

/// Exercise the production RA adapter and added-side diff producer at one line.
fn probes_at(source: &str, line: usize) -> Result<Vec<Probe>, String> {
    let path = PathBuf::from("src/lib.rs");
    let text = source
        .lines()
        .nth(line.saturating_sub(1))
        .ok_or_else(|| format!("fixture has no line {line}"))?
        .to_string();
    let facts = RaRustSyntaxAdapter.summarize_file(&path, source)?;
    let index = RustIndex {
        files: BTreeMap::from([(path.clone(), facts)]),
        ..RustIndex::default()
    };
    let changed = ChangedFile {
        path,
        added_lines: vec![ChangedLine {
            line,
            new_side_line: line,
            text,
        }],
        removed_lines: Vec::new(),
    };
    Ok(probes_for_file(Path::new("."), &changed, &index))
}

#[test]
fn parameter_declaration_retains_unknown_instead_of_field_construction() -> Result<(), String> {
    let probes = probes_at(SOURCE, 4)?;
    assert!(
        !probes.is_empty(),
        "a changed parameter must not disappear from analysis"
    );
    assert!(
        probes
            .iter()
            .all(|probe| probe.family == ProbeFamily::StaticUnknown),
        "parameter syntax is not executable field construction: {probes:?}"
    );
    assert!(
        probes
            .iter()
            .any(|probe| probe.expression.contains("out: &Path"))
    );
    Ok(())
}

#[test]
fn identical_text_in_record_expression_keeps_field_construction() -> Result<(), String> {
    let probes = probes_at(SOURCE, 7)?;
    assert!(
        probes.iter().any(|probe| {
            probe.family == ProbeFamily::FieldConstruction
                && probe.expression.contains("out: &Path")
        }),
        "a real field initializer must not be hidden by parameter filtering: {probes:?}"
    );
    Ok(())
}

#[test]
fn shared_signature_line_keeps_real_body_probe() -> Result<(), String> {
    let source = "struct Path;\nstruct Envelope<'a> { out: &'a Path }\nfn project(out: &Path) -> Envelope<'_> { Envelope { out: &Path } }\n";
    let probes = probes_at(source, 3)?;
    assert!(
        probes.iter().any(|probe| {
            probe.family == ProbeFamily::FieldConstruction
                && probe.expression.contains("out: &Path")
        }),
        "a parameter sharing the line must not erase the real body field: {probes:?}"
    );
    Ok(())
}

#[test]
fn commented_parameter_and_identical_record_text_keep_distinct_families() -> Result<(), String> {
    for suffix in [" // output", " /* output */"] {
        let source = SOURCE.replace("out: &Path,", &format!("out: &Path,{suffix}"));
        let parameters = probes_at(&source, 4)?;
        assert!(!parameters.is_empty());
        assert!(
            parameters
                .iter()
                .all(|probe| probe.family == ProbeFamily::StaticUnknown),
            "comment trivia restored false executable field probes: {parameters:?}"
        );
        assert!(
            parameters
                .iter()
                .any(|probe| probe.expression == format!("out: &Path,{suffix}"))
        );
        let fields = probes_at(&source, 7)?;
        assert!(
            fields
                .iter()
                .any(|probe| probe.family == ProbeFamily::FieldConstruction),
            "the comment fix hid a genuine field: {fields:?}"
        );
    }
    Ok(())
}

#[test]
fn typed_receiver_parameters_remain_explicit_unknowns() -> Result<(), String> {
    for receiver in [
        "self: Box<Self>,",
        "self: &Self, // receiver",
        "self: &mut Self,",
    ] {
        let source = format!(
            "struct Envelope;\nimpl Envelope {{\n    fn project(\n        {receiver}\n    ) {{}}\n}}\n"
        );
        let probes = probes_at(&source, 4)?;
        assert!(!probes.is_empty());
        assert!(
            probes
                .iter()
                .all(|probe| probe.family == ProbeFamily::StaticUnknown),
            "typed receiver was classified as executable: {probes:?}"
        );
        assert!(probes.iter().any(|probe| probe.expression == receiver));
    }
    Ok(())
}

#[test]
fn unsafe_parameter_retains_both_declaration_and_boundary_identity() -> Result<(), String> {
    let source =
        "struct Path;\nunsafe fn project(\n    out: &Path, // preserve both subjects\n) {}\n";
    let path = PathBuf::from("src/lib.rs");
    let facts = RaRustSyntaxAdapter.summarize_file(&path, source)?;
    let boundary = facts
        .probe_shapes
        .iter()
        .find(|shape| shape.kind == PROBE_SHAPE_UNSAFE_BOUNDARY)
        .cloned()
        .ok_or_else(|| "fixture has no unsafe boundary".to_string())?;
    let index = RustIndex {
        files: BTreeMap::from([(path.clone(), facts)]),
        ..RustIndex::default()
    };
    let text = "out: &Path, // preserve both subjects";
    let shapes = parser_probe_shapes_for_changed_line(&index, &path, 3, text);
    assert_eq!(
        shapes.len(),
        2,
        "both independent identities must survive: {shapes:?}"
    );
    let declaration = shapes
        .iter()
        .find(|shape| !shape.unsafe_boundary)
        .ok_or_else(|| "unsafe boundary hid the exact declaration".to_string())?;
    assert_eq!(declaration.family, ProbeFamily::StaticUnknown);
    assert_eq!(declaration.start_line, 3);
    assert_eq!(Some(declaration.start_byte), source.find("out: &Path"));
    assert_eq!(declaration.text, text);
    let retained = shapes
        .iter()
        .find(|shape| shape.unsafe_boundary)
        .ok_or_else(|| "declaration fix erased the unsafe obligation".to_string())?;
    assert_eq!(retained.family, ProbeFamily::StaticUnknown);
    assert_eq!(retained.start_byte, boundary.start_byte);
    assert_eq!(retained.text, boundary.text);
    assert_eq!(retained.start_line, 3);
    let probes = probes_at(source, 3)?;
    assert!(probes.iter().any(|probe| probe.expression == text));
    assert!(probes.iter().any(|probe| probe.expression == boundary.text));
    Ok(())
}

#[test]
fn unsafe_body_without_parameter_keeps_its_existing_obligation() -> Result<(), String> {
    let source = "unsafe fn project() {\n    let value = 7;\n}\n";
    let path = PathBuf::from("src/lib.rs");
    let facts = RaRustSyntaxAdapter.summarize_file(&path, source)?;
    let index = RustIndex {
        files: BTreeMap::from([(path.clone(), facts)]),
        ..RustIndex::default()
    };
    let shapes = parser_probe_shapes_for_changed_line(&index, &path, 2, "let value = 7;");
    assert_eq!(shapes.len(), 1);
    assert!(shapes[0].unsafe_boundary);
    assert_eq!(shapes[0].family, ProbeFamily::StaticUnknown);
    assert_eq!(shapes[0].start_line, 2);
    assert_eq!(Some(shapes[0].start_byte), source.find("unsafe fn"));
    Ok(())
}
