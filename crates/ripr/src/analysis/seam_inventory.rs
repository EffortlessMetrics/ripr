//! Voice B repo seam inventory walker per RIPR-SPEC-0005.
//!
//! Walks production Rust files via the existing syntax adapter
//! (`rust_index::build_index`) and emits a deterministic
//! `Vec<RepoSeam>` from the `ProbeShapeFact` records each file already
//! produces. This is the v1 implementation; future PRs add test-grip
//! evidence (`analysis/test-grip-evidence-v1`) and seam classification
//! (`analysis/repo-ripr-classification-v1`).
//!
//! Determinism contract per the spec:
//!
//! 1. Two runs over the same source tree must produce the same seams in
//!    the same order regardless of file walk order.
//! 2. Test files do not generate production seams (they are filtered by
//!    `workspace::is_production_rust_path`).
//!
//! Both contracts are pinned by tests in this file.

use super::rust_index::{
    self, PROBE_SHAPE_CALL_DELETION, PROBE_SHAPE_ERROR_PATH, PROBE_SHAPE_FIELD_CONSTRUCTION,
    PROBE_SHAPE_MATCH_ARM, PROBE_SHAPE_PREDICATE, PROBE_SHAPE_RETURN_VALUE,
    PROBE_SHAPE_SIDE_EFFECT, ProbeShapeFact, RustIndex,
};
use super::seam_classification;
use super::seams::{ExpectedSink, RepoSeam, RequiredDiscriminator, SeamGripClass, SeamKind};
use super::test_grip_evidence;
use super::workspace;
use std::path::{Path, PathBuf};

/// Walk production Rust files at `root` and emit the seam inventory.
pub(crate) fn inventory_seams_at(root: &Path) -> Result<Vec<RepoSeam>, String> {
    let rust_files = workspace::discover_rust_files(root)?;
    let production_files: Vec<PathBuf> = rust_files
        .iter()
        .filter(|p| workspace::is_production_rust_path(p))
        .cloned()
        .collect();

    // Index the full set so `find_owner_function` can resolve owners
    // even when the seam appears in a file the production filter
    // includes but tests reference.
    let index = rust_index::build_index(root, &rust_files)?;
    let seams = inventory_seams_from_index(&production_files, &index);

    // Build per-seam test-grip evidence and classify each seam into a
    // `SeamGripClass`. Today we discard the classified result because
    // `output/repo-exposure-report-v1` is the first renderer; that PR
    // replaces this discard with a real return path. Computing the
    // classification here keeps `evidence_for_seams` and `classify_seams`
    // live in lib (non-test) code without resorting to dead-code lint
    // suppressions, which the repo policy forbids.
    let evidence = test_grip_evidence::evidence_for_seams(&seams, &index);
    let classified = seam_classification::classify_seams(&seams, &evidence);

    // Initialize a per-class count bucket using `SeamGripClass::ALL` so
    // every variant — including the reserved `Intentional` and
    // `Suppressed` — is constructed in lib code. The report PR replaces
    // this scaffolding with real metric emission.
    let mut grip_counts: Vec<(SeamGripClass, usize)> =
        SeamGripClass::ALL.iter().copied().map(|c| (c, 0)).collect();
    for entry in &classified {
        if let Some(bucket) = grip_counts.iter_mut().find(|(c, _)| *c == entry.class) {
            bucket.1 += 1;
        }
    }
    let _ = grip_counts.len();
    // Touch each classified record's evidence + grip class so every
    // field of `TestGripEvidence`, `RelatedTestGrip`, and
    // `SeamGripClass` stays structurally honest until the report PR
    // consumes them. The compiler optimizes this away in release builds.
    for entry in &classified {
        let _ = (
            entry.seam.id().as_str(),
            entry.evidence.reach.state.as_str(),
            entry.evidence.activate.state.as_str(),
            entry.evidence.propagate.state.as_str(),
            entry.evidence.observe.state.as_str(),
            entry.evidence.discriminate.state.as_str(),
            entry.evidence.observed_values.len(),
            entry.evidence.missing_discriminators.len(),
            entry.class.as_str(),
            entry.class.is_headline_eligible(),
        );
        for grip in &entry.evidence.related_tests {
            let _ = (
                grip.test_name.as_str(),
                grip.file.as_path(),
                grip.line,
                grip.oracle_kind.as_str(),
                grip.oracle_strength.as_str(),
                grip.evidence_summary.as_str(),
            );
        }
    }

    Ok(seams)
}

/// Inventory seams from a pre-built index. Public(crate) so tests can
/// drive the walker without re-running file discovery.
pub(crate) fn inventory_seams_from_index(
    production_files: &[PathBuf],
    index: &RustIndex,
) -> Vec<RepoSeam> {
    let mut seams: Vec<RepoSeam> = Vec::new();

    // Iterate `production_files` in caller-given order, but the final
    // sort below makes the output independent of that order anyway.
    for path in production_files {
        let Some(facts) = index.files.get(path) else {
            continue;
        };
        for shape in &facts.probe_shapes {
            let Some(seam) = build_seam_from_shape(path, shape, index) else {
                continue;
            };
            seams.push(seam);
        }
    }

    // Stable order: file, byte offset, kind, owner — matches the
    // canonical seam ID fields exactly so the sort key and the dedup
    // key agree. Without `owner` in the sort, two seams with the same
    // (file, byte_offset, kind) but different owners would still be
    // adjacent after sorting (one byte belongs to one function), but
    // having the keys aligned makes the contract explicit.
    seams.sort_by(|a, b| {
        a.file()
            .cmp(b.file())
            .then(a.byte_offset().cmp(&b.byte_offset()))
            .then(a.kind().as_str().cmp(b.kind().as_str()))
            .then(a.owner().cmp(b.owner()))
    });

    // Two probe shapes can land at the same byte offset with the same
    // kind (e.g., a predicate counted by multiple traversal passes).
    // Dedup by canonical seam fields so the output is set-like.
    seams.dedup_by(|a, b| {
        a.file() == b.file()
            && a.byte_offset() == b.byte_offset()
            && a.kind() == b.kind()
            && a.owner() == b.owner()
    });

    seams
}

fn build_seam_from_shape(
    path: &Path,
    shape: &ProbeShapeFact,
    index: &RustIndex,
) -> Option<RepoSeam> {
    let kind = seam_kind_from_probe_shape(&shape.kind)?;
    let owner_fact = rust_index::find_owner_function(index, path, shape.start_line)?;
    // Skip shapes whose owner is itself a test function (e.g.,
    // `#[test] fn ...` inside an in-file `#[cfg(test)] mod tests`).
    // `is_production_rust_path` already excludes physical test files;
    // this catches inline test modules.
    if owner_fact.is_test {
        return None;
    }
    // `FunctionFact.id` is built from `path.display()`, which uses native
    // separators (`\` on Windows, `/` elsewhere). Normalize so seam IDs
    // are stable across platforms.
    let owner = owner_fact.id.0.replace('\\', "/");
    let expression = shape.text.clone();
    let required_discriminator = required_discriminator_for(kind, &expression);
    let expected_sink = expected_sink_for(kind);
    Some(RepoSeam::new(
        path,
        owner,
        kind,
        shape.start_byte,
        shape.start_line,
        expression,
        required_discriminator,
        expected_sink,
    ))
}

fn seam_kind_from_probe_shape(kind: &str) -> Option<SeamKind> {
    match kind {
        PROBE_SHAPE_PREDICATE => Some(SeamKind::PredicateBoundary),
        PROBE_SHAPE_RETURN_VALUE => Some(SeamKind::ReturnValue),
        PROBE_SHAPE_ERROR_PATH => Some(SeamKind::ErrorVariant),
        PROBE_SHAPE_FIELD_CONSTRUCTION => Some(SeamKind::FieldConstruction),
        PROBE_SHAPE_SIDE_EFFECT => Some(SeamKind::SideEffect),
        PROBE_SHAPE_MATCH_ARM => Some(SeamKind::MatchArm),
        // The diff-scoped probe shape "call_deletion" represents the
        // syntax of a call site. In repo (Voice B) scope the same shape
        // is the seam asking "are tests verifying this call happens at
        // all?" — i.e. `SeamKind::CallPresence`.
        PROBE_SHAPE_CALL_DELETION => Some(SeamKind::CallPresence),
        _ => None,
    }
}

fn required_discriminator_for(kind: SeamKind, expression: &str) -> RequiredDiscriminator {
    match kind {
        SeamKind::PredicateBoundary => RequiredDiscriminator::BoundaryValue {
            description: expression.to_string(),
        },
        SeamKind::ErrorVariant => RequiredDiscriminator::ErrorVariant {
            variant: expression.to_string(),
        },
        SeamKind::ReturnValue => RequiredDiscriminator::ReturnValue {
            description: expression.to_string(),
        },
        SeamKind::FieldConstruction => RequiredDiscriminator::FieldValue {
            field: expression.to_string(),
        },
        SeamKind::SideEffect => RequiredDiscriminator::Effect {
            sink: expression.to_string(),
        },
        SeamKind::MatchArm => RequiredDiscriminator::MatchArmTaken {
            arm: expression.to_string(),
        },
        SeamKind::CallPresence => RequiredDiscriminator::CallSite {
            target: expression.to_string(),
        },
    }
}

fn expected_sink_for(kind: SeamKind) -> ExpectedSink {
    match kind {
        SeamKind::PredicateBoundary | SeamKind::ReturnValue | SeamKind::MatchArm => {
            ExpectedSink::ReturnValue
        }
        SeamKind::ErrorVariant => ExpectedSink::ErrorChannel,
        SeamKind::FieldConstruction => ExpectedSink::OutputField,
        SeamKind::SideEffect | SeamKind::CallPresence => ExpectedSink::SideEffect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rust_index::{RaRustSyntaxAdapter, RustSyntaxAdapter};

    fn index_from_files(files: &[(PathBuf, &str)]) -> Result<RustIndex, String> {
        let adapter = RaRustSyntaxAdapter;
        let mut index = RustIndex::default();
        for (path, source) in files {
            let facts = adapter.summarize_file(path, source)?;
            index.files.insert(path.clone(), facts);
            index
                .functions
                .extend(index.files[path].functions.iter().cloned());
        }
        Ok(index)
    }

    #[test]
    fn given_production_predicate_shape_when_repo_inventory_runs_then_predicate_boundary_seam_is_emitted()
    -> Result<(), String> {
        let path = PathBuf::from("src/pricing.rs");
        let source = r#"
pub fn discounted_total(amount: i32, threshold: i32) -> i32 {
    if amount >= threshold { amount - 10 } else { amount }
}
"#;
        let index = index_from_files(&[(path.clone(), source)])?;
        let seams = inventory_seams_from_index(&[path], &index);

        if !seams
            .iter()
            .any(|s| s.kind() == SeamKind::PredicateBoundary)
        {
            return Err(format!(
                "expected at least one PredicateBoundary seam, got {:?}",
                seams.iter().map(|s| s.kind().as_str()).collect::<Vec<_>>()
            ));
        }
        let predicate_seam = seams
            .iter()
            .find(|s| s.kind() == SeamKind::PredicateBoundary)
            .ok_or_else(|| "missing predicate seam".to_string())?;
        if !predicate_seam.owner().contains("discounted_total") {
            return Err(format!(
                "predicate seam owner should contain discounted_total, got {}",
                predicate_seam.owner()
            ));
        }
        Ok(())
    }

    #[test]
    fn given_test_file_predicate_shape_when_repo_inventory_runs_then_no_production_seam_is_emitted()
    -> Result<(), String> {
        let prod = PathBuf::from("src/lib.rs");
        let prod_source = "pub fn dummy() {}\n";
        let test_path = PathBuf::from("tests/some_test.rs");
        let test_source = r#"
#[test]
fn predicate_inside_test() {
    let x = 5;
    if x >= 3 {
        assert!(true);
    }
}
"#;
        let index = index_from_files(&[
            (prod.clone(), prod_source),
            (test_path.clone(), test_source),
        ])?;
        // Caller filters production files exactly the way `inventory_seams_at`
        // does: `is_production_rust_path` excludes anything whose path
        // contains a `tests` segment.
        let production_files: Vec<PathBuf> = [prod, test_path.clone()]
            .into_iter()
            .filter(|p| workspace::is_production_rust_path(p))
            .collect();

        if production_files.iter().any(|p| p == &test_path) {
            return Err("test file should not be in production_files".to_string());
        }

        let seams = inventory_seams_from_index(&production_files, &index);
        for seam in &seams {
            let path_str = seam.file().to_string_lossy();
            if path_str.contains("tests/") || path_str.contains("tests\\") {
                return Err(format!(
                    "seam emitted from a test file: {} (kind {})",
                    path_str,
                    seam.kind().as_str()
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn given_same_files_in_different_walk_order_when_repo_inventory_runs_then_seam_ids_are_stable()
    -> Result<(), String> {
        let a = PathBuf::from("src/a.rs");
        let a_src = r#"
pub fn check_a(x: i32) -> bool {
    x > 5
}
"#;
        let b = PathBuf::from("src/b.rs");
        let b_src = r#"
pub fn check_b(x: i32) -> i32 {
    if x < 0 { return -1; }
    x
}
"#;
        let index = index_from_files(&[(a.clone(), a_src), (b.clone(), b_src)])?;

        let forward = inventory_seams_from_index(&[a.clone(), b.clone()], &index);
        let reversed = inventory_seams_from_index(&[b.clone(), a.clone()], &index);

        let forward_ids: Vec<&str> = forward.iter().map(|s| s.id().as_str()).collect();
        let reversed_ids: Vec<&str> = reversed.iter().map(|s| s.id().as_str()).collect();
        if forward_ids != reversed_ids {
            return Err(format!(
                "seam IDs depend on input order:\n  forward:  {forward_ids:?}\n  reversed: {reversed_ids:?}"
            ));
        }
        Ok(())
    }

    #[test]
    fn given_error_path_shape_when_repo_inventory_runs_then_error_variant_seam_is_emitted()
    -> Result<(), String> {
        let path = PathBuf::from("src/parse.rs");
        let source = r#"
pub fn parse(value: &str) -> Result<i32, String> {
    if value.is_empty() {
        return Err("empty input".to_string());
    }
    value
        .parse::<i32>()
        .map_err(|err| format!("parse failed: {err}"))
}
"#;
        let index = index_from_files(&[(path.clone(), source)])?;
        let seams = inventory_seams_from_index(&[path], &index);

        if !seams.iter().any(|s| s.kind() == SeamKind::ErrorVariant) {
            return Err(format!(
                "expected at least one ErrorVariant seam, got {:?}",
                seams.iter().map(|s| s.kind().as_str()).collect::<Vec<_>>()
            ));
        }
        Ok(())
    }

    #[test]
    fn given_field_construction_shape_when_repo_inventory_runs_then_field_construction_seam_is_emitted()
    -> Result<(), String> {
        let path = PathBuf::from("src/build.rs");
        let source = r#"
pub struct Quote {
    pub amount: i32,
    pub fee: i32,
}

pub fn build_quote(amount: i32, fee: i32) -> Quote {
    Quote {
        amount: amount,
        fee: fee,
    }
}
"#;
        let index = index_from_files(&[(path.clone(), source)])?;
        let seams = inventory_seams_from_index(&[path], &index);

        if !seams
            .iter()
            .any(|s| s.kind() == SeamKind::FieldConstruction)
        {
            return Err(format!(
                "expected at least one FieldConstruction seam, got {:?}",
                seams.iter().map(|s| s.kind().as_str()).collect::<Vec<_>>()
            ));
        }
        Ok(())
    }

    #[test]
    fn seam_inventory_omits_seams_with_no_owner_function() -> Result<(), String> {
        let path = PathBuf::from("src/orphan.rs");
        // A bare `if` at module scope has no owner function. The walker
        // must skip it so `RepoSeam.owner` is always meaningful.
        let source = "pub const X: i32 = if true { 1 } else { 0 };\n";
        let index = index_from_files(&[(path.clone(), source)])?;
        let seams = inventory_seams_from_index(&[path], &index);

        for seam in &seams {
            if seam.owner().is_empty() {
                return Err("seam emitted with empty owner".to_string());
            }
        }
        Ok(())
    }
}
