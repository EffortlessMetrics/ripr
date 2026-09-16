//! #1453: identical text is a parameter declaration or a field initializer
//! depending on parser-owned context. Do not erase the parameter change or
//! credit it as exposed; retain an explicit unknown rather than a false field.

use super::diff::probes_for_file;
use crate::analysis::diff::{ChangedFile, ChangedLine};
use crate::analysis::rust_index::RustIndex;
use crate::analysis::syntax::{RaRustSyntaxAdapter, RustSyntaxAdapter};
use crate::domain::{Probe, ProbeFamily};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SOURCE: &str = "struct Path;\nstruct Envelope<'a> { out: &'a Path }\nfn project(\n    out: &Path,\n) -> Envelope<'_> {\n    Envelope {\n        out: &Path,\n    }\n}\n";

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
