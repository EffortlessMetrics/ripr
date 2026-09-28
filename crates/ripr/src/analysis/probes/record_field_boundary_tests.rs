//! Record-field declarations and initializers require distinct source identities.
//! Regression for EffortlessMetrics/ripr#1453 and ub-review#1306.

use super::classify::parser_probe_shapes_for_changed_line;
use super::diff::probes_for_file;
use crate::analysis::diff::{ChangedFile, ChangedLine};
use crate::analysis::extract::PROBE_SHAPE_UNSAFE_BOUNDARY;
use crate::analysis::rust_index::RustIndex;
use crate::analysis::syntax::{RaRustSyntaxAdapter, RustSyntaxAdapter};
use crate::domain::{Probe, ProbeFamily};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SOURCE: &str = "struct Marker;\nstruct Packet {\n    value: Marker,\n}\nfn packet() -> Packet {\n    Packet {\n        value: Marker,\n    }\n}\n";

/// Build the real RA summary and diff-produced probes for one exact changed line.
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
fn record_field_declaration_retains_exact_unknown_subject() -> Result<(), String> {
    let probes = probes_at(SOURCE, 3)?;
    assert_eq!(
        probes.len(),
        1,
        "declaration must remain visible: {probes:?}"
    );
    assert_eq!(
        probes[0].family,
        ProbeFamily::StaticUnknown,
        "a field declaration is not an executable initializer"
    );
    assert_eq!(probes[0].location.line, 3);
    assert_eq!(probes[0].expression, "value: Marker,");
    Ok(())
}

#[test]
fn identical_record_initializer_retains_executable_subject() -> Result<(), String> {
    let probes = probes_at(SOURCE, 7)?;
    assert!(
        probes.iter().any(|probe| {
            probe.family == ProbeFamily::FieldConstruction
                && probe.location.line == 7
                && probe.expression.contains("value: Marker")
        }),
        "record declaration handling erased an actual initializer: {probes:?}"
    );
    Ok(())
}

#[test]
fn commented_record_fields_and_initializers_keep_distinct_families() -> Result<(), String> {
    for suffix in [" // field", " /* outer /* nested */ tail */"] {
        for newline in ["\n", "\r\n"] {
            let text = format!("value: Marker,{suffix}");
            let source = SOURCE
                .replace("value: Marker,", &text)
                .replace('\n', newline);
            let declarations = probes_at(&source, 3)?;
            assert_eq!(declarations.len(), 1);
            assert_eq!(declarations[0].family, ProbeFamily::StaticUnknown);
            assert_eq!(declarations[0].location.line, 3);
            assert_eq!(declarations[0].expression, text);
            let initializers = probes_at(&source, 7)?;
            assert!(
                initializers.iter().any(|probe| {
                    probe.family == ProbeFamily::FieldConstruction && probe.location.line == 7
                }),
                "comment handling erased the real initializer: {initializers:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn nested_record_declaration_retains_its_unsafe_boundary() -> Result<(), String> {
    let source = "unsafe fn packet() {\n    struct Packet {\n        value: u8,\n    }\n}\n";
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
    let shapes = parser_probe_shapes_for_changed_line(&index, &path, 3, "value: u8,");
    assert_eq!(shapes.len(), 2);
    let declaration = shapes
        .iter()
        .find(|shape| !shape.unsafe_boundary)
        .ok_or_else(|| "unsafe boundary hid the declaration".to_string())?;
    assert_eq!(declaration.family, ProbeFamily::StaticUnknown);
    assert_eq!(declaration.start_line, 3);
    assert_eq!(Some(declaration.start_byte), source.find("value: u8,"));
    assert_eq!(declaration.text, "value: u8,");
    let retained = shapes
        .iter()
        .find(|shape| shape.unsafe_boundary)
        .ok_or_else(|| "record field handling erased the unsafe boundary".to_string())?;
    assert_eq!(retained.start_byte, boundary.start_byte);
    assert_eq!(retained.text, boundary.text);
    let probes = probes_at(source, 3)?;
    assert!(probes.iter().any(|probe| probe.expression == "value: u8,"));
    assert!(probes.iter().any(|probe| probe.expression == boundary.text));
    Ok(())
}

#[test]
fn shared_record_definition_and_body_keep_the_real_initializer() -> Result<(), String> {
    let source = "struct Marker;\nstruct Packet { value: Marker } fn packet() -> Packet { Packet { value: Marker } }\n";
    let probes = probes_at(source, 2)?;
    assert!(
        probes.iter().any(|probe| {
            probe.family == ProbeFamily::FieldConstruction
                && probe.expression.contains("value: Marker")
        }),
        "shared declaration line erased executable field evidence: {probes:?}"
    );
    Ok(())
}
