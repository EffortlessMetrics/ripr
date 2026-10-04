use super::*;
use crate::analysis::syntax::{RaRustSyntaxAdapter, RustSyntaxAdapter};

fn large_file() -> Result<FileFacts, String> {
    let body = (0..256)
        .map(|n| {
            format!(
                "let value_{n} = value(\"payload_{n:04}_{}\"); assert_eq!(value_{n}, {n});\n",
                "x".repeat(128)
            )
        })
        .collect::<String>();
    RaRustSyntaxAdapter.summarize_file(
        Path::new("tests/large.rs"),
        &format!(
            "fn value(input: &str) -> usize {{ input.len() }}\n#[test]\nfn large() {{\n{body}}}\n"
        ),
    )
}

#[test]
fn moved_large_payload_has_one_backing_through_both_ordered_views() -> Result<(), String> {
    let facts = large_file()?;
    assert!(facts.tests[0].body.len() > 32_768);
    assert!(facts.tests[0].calls.len() >= 256);
    assert!(facts.tests[0].assertions.len() >= 256);
    assert!(facts.tests[0].let_bindings.len() >= 256);
    let body = facts.tests[0].body.as_ptr();
    let calls = facts.tests[0].calls.as_ptr();
    let assertions = facts.tests[0].assertions.as_ptr();
    let mut index = RustIndex::default();
    let path = PathBuf::from("tests/large.rs");
    index.insert_file(path.clone(), facts, true);
    index.finalize()?;
    let file = index.files().get(&path).ok_or("missing large file")?;
    assert_eq!(index.function_facts.len(), index.functions().len());
    assert_eq!(index.test_facts.len(), index.tests().len());
    for (flat, local) in index.functions().iter().zip(file.functions.iter()) {
        assert!(std::ptr::eq(flat, local));
        assert_eq!(flat.body.as_ptr(), local.body.as_ptr());
        assert_eq!(flat.calls.as_ptr(), local.calls.as_ptr());
    }
    assert!(std::ptr::eq(index.tests().at(0), &file.tests[0]));
    assert_eq!(index.tests()[0].body.as_ptr(), body);
    assert_eq!(index.tests()[0].calls.as_ptr(), calls);
    assert_eq!(index.tests()[0].assertions.as_ptr(), assertions);
    Ok(())
}

#[test]
fn union_mutation_visits_shared_flat_only_and_file_only_once() -> Result<(), String> {
    let facts = large_file()?;
    let mut index = RustIndex::default();
    index.push_test(facts.tests[0].clone());
    index.insert_file(PathBuf::from("file-only.rs"), facts.clone(), false);
    index.insert_file(PathBuf::from("shared.rs"), facts, true);
    let count = index.test_facts.len();
    assert_eq!(count, 3);
    let mut visits = 0;
    index.for_each_test_mut(|test| {
        visits += 1;
        test.body.push_str("// once");
    });
    assert_eq!(visits, count);
    for test in index.test_facts.iter() {
        assert_eq!(test.body.matches("// once").count(), 1);
    }
    let shared = index
        .files()
        .get(Path::new("shared.rs"))
        .ok_or("missing shared file")?;
    assert!(std::ptr::eq(index.tests().at(1), &shared.tests[0]));
    Ok(())
}

#[test]
fn compaction_keeps_independent_membership_and_duplicate_occurrences() -> Result<(), String> {
    let facts = large_file()?;
    let mut index = RustIndex::default();
    index.insert_file(PathBuf::from("shared.rs"), facts.clone(), true);
    index.insert_file(PathBuf::from("local.rs"), facts.clone(), false);
    index.insert_file(PathBuf::from("local.rs"), facts, false);
    assert_eq!(index.test_facts.len(), 3);
    index.remove_file(Path::new("shared.rs"));
    index.finalize()?;
    assert_eq!(index.tests().len(), 1);
    assert_eq!(index.test_facts.len(), 2);
    let local = index
        .files()
        .get(Path::new("local.rs"))
        .ok_or("missing local file")?;
    assert_eq!(index.tests().at(0), &local.tests[0]);
    assert!(!std::ptr::eq(index.tests().at(0), &local.tests[0]));
    assert_eq!(index.tests()[0].file, Path::new("tests/large.rs"));
    Ok(())
}

#[test]
fn whole_index_wire_stays_expanded_and_cloning_does_not_pin_a_generation() -> Result<(), String> {
    let facts = large_file()?;
    let mut index = RustIndex::default();
    let path = PathBuf::from("tests/large.rs");
    index.insert_file(path.clone(), facts.clone(), true);
    let expected = OwnedRustIndex {
        functions: facts.functions.clone(),
        tests: facts.tests.clone(),
        files: BTreeMap::from([(path.clone(), facts)]),
        ..OwnedRustIndex::default()
    };
    let wire = serde_json::to_value(&index).map_err(|e| e.to_string())?;
    assert_eq!(
        wire,
        serde_json::to_value(&expected).map_err(|e| e.to_string())?
    );
    let decoded: RustIndex = serde_json::from_value(wire.clone()).map_err(|e| e.to_string())?;
    assert_eq!(
        serde_json::to_value(&decoded).map_err(|e| e.to_string())?,
        wire
    );
    let cloned = index.clone();
    assert!(!std::ptr::eq(index.tests().at(0), &cloned.tests()[0]));
    assert_ne!(
        index.tests()[0].body.as_ptr(),
        cloned.tests()[0].body.as_ptr()
    );
    drop(index);
    let local = cloned.files().get(&path).ok_or("missing cloned file")?;
    assert!(std::ptr::eq(&cloned.tests()[0], &local.tests[0]));
    Ok(())
}

#[test]
fn invalid_membership_fails_finalization_before_compaction() -> Result<(), String> {
    let mut index = RustIndex::default();
    index.insert_file(PathBuf::from("large.rs"), large_file()?, true);
    let original_count = index.function_facts.len();
    index.function_order.push(FactId::new(original_count));
    let error = index
        .finalize()
        .err()
        .ok_or("invalid membership was accepted")?;
    assert!(error.starts_with("invalid function fact membership:"));
    assert_eq!(index.function_facts.len(), original_count);
    Ok(())
}

#[test]
fn finalization_observes_deadline_at_return_after_compaction_and_membership_refresh()
-> Result<(), String> {
    use crate::analysis::cancellation::{AnalysisCancellationToken, with_token};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use std::time::{Duration, Instant};

    let path = PathBuf::from("tests/surviving.rs");
    let facts = RaRustSyntaxAdapter.summarize_file(&path, "#[test]\nfn surviving() {}\n")?;
    assert_eq!(facts.functions.len(), 1);
    assert_eq!(facts.tests.len(), 1);
    let mut candidate = RustIndex::default();
    candidate.insert_file(PathBuf::from("discard.rs"), facts.clone(), false);
    candidate.insert_file(path.clone(), facts, true);
    assert!(candidate.remove_file(Path::new("discard.rs")));
    assert_eq!(candidate.function_facts.len(), 2);
    assert_eq!(candidate.test_facts.len(), 2);
    assert_eq!(candidate.functions().len(), 1);
    assert_eq!(candidate.tests().len(), 1);

    let calls = Arc::new(AtomicUsize::new(0));
    let clock_calls = Arc::clone(&calls);
    let started = Instant::now();
    let token = AnalysisCancellationToken::with_budget(
        started,
        Duration::from_millis(1),
        Arc::new(move || {
            if clock_calls.fetch_add(1, Ordering::SeqCst) < 11 {
                started
            } else {
                started + Duration::from_millis(1)
            }
        }),
    );
    // This fixed nonempty shape reaches entry (1), validation (2-3),
    // pre-compaction (4), both compacted arenas (5-6), refresh entry (7),
    // validation (8-9), reverse memberships (10-11), and return (12).
    // Do not calibrate this threshold from a successful run: removing the
    // return checkpoint must turn this result into Ok and fail the oracle.
    let result = with_token(&token, || candidate.finalize());
    assert_eq!(
        result.err().as_deref(),
        Some("analysis cancelled: DeadlineExceeded")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 12);
    assert_eq!(candidate.function_facts.len(), 1);
    assert_eq!(candidate.test_facts.len(), 1);
    assert_eq!(
        candidate.function_slot(candidate.functions().at(0)),
        Some(0)
    );
    assert_eq!(candidate.test_slot(candidate.tests().at(0)), Some(0));
    let local = candidate
        .files()
        .get(&path)
        .ok_or("missing surviving file")?;
    assert!(std::ptr::eq(
        candidate.functions().at(0),
        local.functions.at(0)
    ));
    assert!(std::ptr::eq(candidate.tests().at(0), local.tests.at(0)));
    Ok(())
}

#[test]
fn shared_harness_demotion_splits_only_disagreeing_containing_paths() -> Result<(), String> {
    let target = Path::new("tests/container.rs");
    for fact_path in [Path::new("src/claimed.rs"), target] {
        let facts = RaRustSyntaxAdapter.summarize_file(fact_path, "#[test]\nfn subject() {}\n")?;
        assert_eq!(facts.functions.len(), 1);
        assert_eq!(facts.tests.len(), 1);
        assert_eq!(facts.functions[0].file, fact_path);
        assert_eq!(facts.tests[0].file, fact_path);
        assert_eq!(
            facts.functions[0].source_role,
            FunctionSourceRole::TestAttribute
        );
        let mut index = RustIndex::default();
        index.insert_file(target.to_path_buf(), facts, true);
        let local = index.files().get(target).ok_or("missing harness target")?;
        assert!(std::ptr::eq(index.functions().at(0), local.functions.at(0)));
        assert!(std::ptr::eq(index.tests().at(0), local.tests.at(0)));
        assert_eq!(index.function_facts.len(), 1);
        assert_eq!(index.test_facts.len(), 1);

        super::super::harness_registry::demote_harness_target_functions(&mut index, target);
        index.finalize()?;
        let local = index.files().get(target).ok_or("missing demoted target")?;
        assert_eq!(local.functions.len(), 1);
        assert_eq!(index.functions().len(), 1);
        assert_eq!(
            local.functions[0].source_role,
            FunctionSourceRole::HarnessHelper
        );
        assert!(local.tests.is_empty());
        if fact_path == target {
            // Aligned selection must mutate the shared occurrence in place,
            // rather than unconditionally clone for the two ordered views.
            assert_eq!(
                index.functions()[0].source_role,
                FunctionSourceRole::HarnessHelper
            );
            assert!(std::ptr::eq(index.functions().at(0), local.functions.at(0)));
            assert_eq!(index.function_facts.len(), 1);
            assert!(index.tests().is_empty());
            assert!(index.test_facts.is_empty());
        } else {
            // Containing-file selection demotes only the local view. The flat
            // view still follows the fact's own declared path and role.
            assert_eq!(
                index.functions()[0].source_role,
                FunctionSourceRole::TestAttribute
            );
            assert!(!std::ptr::eq(
                index.functions().at(0),
                local.functions.at(0)
            ));
            assert_eq!(index.function_facts.len(), 2);
            assert_eq!(index.tests().len(), 1);
            assert_eq!(index.test_facts.len(), 1);
            assert_eq!(index.tests()[0].file, fact_path);
        }
    }
    Ok(())
}

#[test]
fn flat_slot_identity_survives_reordering_compaction_and_repeated_membership() -> Result<(), String>
{
    let facts = large_file()?;
    let mut index = RustIndex::default();
    index.insert_file(PathBuf::from("discard.rs"), facts.clone(), false);
    index.push_test(facts.tests[0].clone());
    index.push_function(facts.functions[0].clone());
    index.insert_file(PathBuf::from("local.rs"), facts.clone(), false);
    index.insert_file(PathBuf::from("shared.rs"), facts, true);
    let detached = index.tests()[0].clone();
    assert_eq!(index.test_slot(&detached), None);
    let local = index
        .files()
        .get(Path::new("local.rs"))
        .ok_or("missing local file")?;
    assert_eq!(index.test_slot(local.tests.at(0)), None);
    assert_eq!(index.function_slot(local.functions.at(0)), None);
    let duplicate = index.test_order[0];
    index.test_order.push(duplicate);
    index.test_order.reverse();
    index.function_order.reverse();
    index.refresh_memberships()?;
    for (position, test) in index.tests().iter().enumerate() {
        let expected = index
            .tests()
            .iter()
            .position(|candidate| std::ptr::eq(candidate, test));
        assert_eq!(index.test_slot(test), expected, "test position {position}");
    }
    index.remove_file(Path::new("discard.rs"));
    index.finalize()?;
    for (position, function) in index.functions().iter().enumerate() {
        assert_eq!(index.function_slot(function), Some(position));
    }
    for test in index.tests() {
        let expected = index
            .tests()
            .iter()
            .position(|candidate| std::ptr::eq(candidate, test));
        assert_eq!(index.test_slot(test), expected);
    }
    let other = index.clone();
    assert_ne!(index.storage_identity(), other.storage_identity());
    assert_eq!(index.test_slot(other.tests().at(0)), None);
    assert_eq!(index.function_slot(other.functions().at(0)), None);
    Ok(())
}

#[test]
fn fact_view_equality_compares_values_and_order_across_storage_layouts() {
    let dense = [10, 20, 10];
    let arena = [20, 10, 99, 10];
    let order = [FactId::new(1), FactId::new(0), FactId::new(3)];
    let indexed = FactSlice {
        arena: &arena,
        order: Some(&order),
    };
    assert_eq!(FactSlice::from_slice(&dense), indexed);
    assert_eq!(format!("{indexed:?}"), "[10, 20, 10]");
    assert_ne!(FactSlice::from_slice(&[20, 10, 10]), indexed);
    assert_ne!(FactSlice::from_slice(&[10, 20]), indexed);
    assert_ne!(FactSlice::from_slice(&[10, 20, 99]), indexed);
}

#[test]
fn file_view_equality_compares_complete_metadata_and_local_membership() -> Result<(), String> {
    let facts = large_file()?;
    let path = PathBuf::from("tests/large.rs");
    let mut first = RustIndex::default();
    first.insert_file(path.clone(), facts.clone(), true);
    let mut second = RustIndex::default();
    // An unrelated flat-only occurrence shifts the arena offsets, but is not
    // part of the file map's semantic value.
    second.push_function(facts.functions[0].clone());
    second.insert_file(path.clone(), facts, false);
    assert_eq!(first.files(), second.files());
    assert_eq!(first.files().get(&path), second.files().get(&path));
    assert_ne!(first.functions(), second.functions());
    let provenance = second
        .files
        .get(&path)
        .ok_or("missing second file")?
        .role_provenance
        .clone();
    second
        .files
        .get_mut(&path)
        .ok_or("missing second file")?
        .role_provenance
        .earliest_unresolved_reason = Some("unresolved control".into());
    assert_ne!(first.files(), second.files());
    second
        .files
        .get_mut(&path)
        .ok_or("missing second file")?
        .role_provenance = provenance;
    assert_eq!(first.files(), second.files());
    second
        .files
        .get_mut(&path)
        .ok_or("missing second file")?
        .functions
        .reverse();
    assert_ne!(first.files(), second.files());
    Ok(())
}

#[test]
fn unresolved_property_macros_survive_file_view_wire_and_owned_round_trip() -> Result<(), String> {
    let path = PathBuf::from("tests/property.rs");
    let facts = RaRustSyntaxAdapter.summarize_file(
        &path,
        "macro_rules! proptest { ($($t:tt)*) => {} }\nproptest! { #[test] fn phantom() { assert_eq!(gate(10), true); } }\n",
    )?;
    assert_eq!(facts.unresolved_property_macros.len(), 1);
    assert!(facts.tests.is_empty());
    let mut index = RustIndex::default();
    index.insert_file(path.clone(), facts.clone(), true);
    let view = index.files().get(&path).ok_or("missing property file")?;
    assert_eq!(
        view.unresolved_property_macros,
        facts.unresolved_property_macros
    );
    assert_eq!(
        serde_json::to_value(view).map_err(|e| e.to_string())?,
        serde_json::to_value(&facts).map_err(|e| e.to_string())?
    );
    assert_eq!(index.owned_file(&path).as_ref(), Some(&facts));
    let wire = serde_json::to_value(&index).map_err(|e| e.to_string())?;
    let decoded: RustIndex = serde_json::from_value(wire).map_err(|e| e.to_string())?;
    assert_eq!(decoded.owned_file(&path).as_ref(), Some(&facts));
    Ok(())
}
