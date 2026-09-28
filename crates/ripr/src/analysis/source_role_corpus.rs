//! #3534 source-role conformance corpus driver.
//!
//! Each case under `fixtures/source-role-corpus/cases/<name>/` pins the
//! producer-owned source-role facts for a point of the matrix documented in
//! `fixtures/source-role-corpus/README.md`: file roles, executable-test
//! membership, helper demotion, cfg-variant equivalence, and lookalike
//! rejection. The assertions ARE the expected facts; a producer change that
//! flips any of them must be a deliberate spec-level decision.

use crate::analysis::facts::{RustIndex, build_index};
use crate::analysis::rust_index::is_test_file;
use crate::analysis::workspace::SourceRoleContext;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn case_root(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/source-role-corpus/cases")
        .join(name)
}

fn case_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    fn walk(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
        let entries = fs::read_dir(directory)
            .map_err(|error| format!("case directory {directory:?} unreadable: {error}"))?;
        let mut collected: Vec<_> = entries.filter_map(Result::ok).collect();
        collected.sort_by_key(|entry| entry.file_name());
        for entry in collected {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, files)?;
            } else {
                files.push(path);
            }
        }
        Ok(())
    }
    walk(root, &mut files)?;
    Ok(files)
}

fn build_case(name: &str) -> Result<RustIndex, String> {
    let root = case_root(name);
    let files = case_files(&root)?;
    build_index(&root, &files)
}

fn roles_by_name(index: &RustIndex) -> BTreeMap<String, String> {
    index
        .functions
        .iter()
        .map(|function| (function.name.clone(), format!("{:?}", function.source_role)))
        .collect()
}

fn empty_context() -> SourceRoleContext {
    SourceRoleContext::empty()
}

fn test_names(index: &RustIndex) -> Vec<String> {
    let mut names: Vec<String> = index.tests.iter().map(|test| test.name.clone()).collect();
    names.sort();
    names
}

#[test]
fn production_lib_has_one_production_subject_and_no_executable_tests() -> Result<(), String> {
    let index = build_case("production_lib")?;
    let roles = roles_by_name(&index);
    assert_eq!(
        roles.get("discount").map(String::as_str),
        Some("Production")
    );
    assert!(test_names(&index).is_empty());
    assert!(!is_test_file(Path::new("src/lib.rs")));
    let role = crate::analysis::workspace::classify_with(Path::new("src/lib.rs"), &empty_context());
    assert_eq!(format!("{:?}", role), "ProductionSubject");
    Ok(())
}

#[test]
fn explicit_test_target_joins_executable_tests_and_stays_out_of_production() -> Result<(), String> {
    let index = build_case("explicit_test_target")?;
    assert_eq!(test_names(&index), vec!["discount_runs".to_string()]);
    let roles = roles_by_name(&index);
    assert_eq!(
        roles.get("discount").map(String::as_str),
        Some("Production")
    );
    assert!(is_test_file(Path::new("tests/custom/smoke_case.rs")));
    let custom_role = crate::analysis::workspace::classify_with(
        Path::new("tests/custom/smoke_case.rs"),
        &empty_context(),
    );
    assert_eq!(format!("{:?}", custom_role), "TestEvidence");
    let smoke_role =
        crate::analysis::workspace::classify_with(Path::new("tests/smoke.rs"), &empty_context());
    assert_eq!(format!("{:?}", smoke_role), "TestEvidence");
    let lib_role =
        crate::analysis::workspace::classify_with(Path::new("src/lib.rs"), &empty_context());
    assert_eq!(format!("{:?}", lib_role), "ProductionSubject");
    Ok(())
}

#[test]
fn lookalike_test_named_file_in_src_is_a_production_subject() -> Result<(), String> {
    let index = build_case("naming_lookalike")?;
    // `src/price_test.rs` is named like a test file but carries no test
    // attributes and no `tests` root: a plain function there stays a
    // production subject, and the name alone cannot establish role.
    let roles = roles_by_name(&index);
    assert_eq!(
        roles.get("price_probe").map(String::as_str),
        Some("Production")
    );
    assert!(test_names(&index).is_empty(), "{:?}", test_names(&index));
    assert!(!is_test_file(Path::new("src/price_test.rs")));
    Ok(())
}

#[test]
fn cfg_test_variants_all_reach_executable_tests() -> Result<(), String> {
    let index = build_case("cfg_variants")?;
    assert_eq!(
        test_names(&index),
        vec![
            "conjunct_case".to_string(),
            "negated_case".to_string(),
            "plain_case".to_string(),
        ]
    );
    assert!(roles_by_name(&index).contains_key("rate"));
    Ok(())
}
