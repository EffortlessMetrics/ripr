//! Bounded transitive-reach check for Rust `no_static_path` findings.
//!
//! When the direct-call classifier finds no related test (ExposureClass::NoStaticPath),
//! this module runs a depth-bounded breadth-first walk over the lexical call facts
//! in the RustIndex to detect whether any test may plausibly reach the changed owner
//! through a transitive call chain.
//!
//! ## Fail-closed design (RIPR-SPEC-0114)
//!
//! - The walk is pure NAME matching over lexical call facts - no AST resolution.
//! - Depth is bounded at 3 hops (`MAX_TRANSITIVE_DEPTH`).
//! - The walk stops (and NAMES the limitation) at any boundary:
//!   macro invocations (`name!`), callee names not found in the production
//!   function set, or depth > 3.
//! - Finding classification NEVER changes: `no_static_path` stays `no_static_path`.
//!   This check only sets `static_limit_kind` to name the limitation.
//! - If no candidate transitive path is found the finding is left exactly as-is.

use crate::analysis::facts::{CallFact, FunctionSummary, RustIndex, TestFact};
use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;

/// Maximum call-hop depth for the transitive walk.
const MAX_TRANSITIVE_DEPTH: usize = 3;

/// A concrete pointer to the test that witnessed a transitive-reach candidate
/// path, captured so the limitation message can name something the user can
/// open and inspect (RIPR-SPEC-0115).
///
/// This is a *candidate* witness: the test calls `entry_symbol`, an in-crate
/// entry point from which a bounded name-only BFS reaches the changed owner. It
/// is NOT a confirmed reaching test and is deliberately kept out of
/// `related_tests` (the verified-relation channel).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis) struct TransitiveWitness {
    pub test_name: String,
    pub test_file: PathBuf,
    pub test_line: usize,
    /// The non-macro, non-direct callee the test invoked that began the walk
    /// reaching the owner (the "public-API entry point").
    pub entry_symbol: String,
    /// Number of *other* distinct tests (beyond this named one) that also
    /// witnessed a candidate path. Used only to note the count, not enumerate.
    pub other_test_count: usize,
}

/// Finds a deterministic witnessing test for a transitive-reach candidate path
/// to a function named `owner_name`, via a bounded BFS over lexical call facts.
///
/// Returns `Some(witness)` when at least one test reaches the owner (test ->
/// ... -> owner) within `MAX_TRANSITIVE_DEPTH` hops using same-crate production
/// functions only; the named witness is selected deterministically (see below).
/// Returns `None` when no such candidate path exists.
///
/// The caller is responsible for wiring the result into `static_limit_kind`
/// ONLY when the finding's class is `no_static_path` and `related_tests` is
/// empty - i.e. only after the direct-call classifier has already returned
/// empty-handed. Classification NEVER changes.
pub(in crate::analysis) fn find_transitive_witness(
    owner_name: &str,
    index: &RustIndex,
) -> Option<TransitiveWitness> {
    if owner_name.is_empty() {
        return None;
    }

    // Collect all tests from the index (flat tests vec + per-file tests).
    let all_tests = collect_all_tests(index);

    // Build a flat list of production (non-test) function facts for name lookup.
    let prod_fns: Vec<&FunctionSummary> = index
        .files
        .values()
        .flat_map(|file| file.functions.iter().filter(|f| !f.is_test))
        .collect();

    // One witness per test: the lexicographically-smallest entry symbol from
    // that test which reaches the owner. Collected as a sortable 4-tuple so the
    // named witness is stable across index iteration order (goldens depend on
    // this determinism).
    let mut witnesses: Vec<(PathBuf, usize, String, String)> = Vec::new();
    for test in &all_tests {
        let mut entry: Option<&str> = None;
        for callee in &test.calls {
            // Skip macro invocations.
            if is_macro_call(&callee.name) {
                continue;
            }
            // Skip direct calls to the owner - the direct-call classifier
            // already handles that case (and would have found the test).
            if callee.name == owner_name {
                continue;
            }
            // BFS from this callee through the production call graph.
            if bfs_reaches_owner(&callee.name, owner_name, &prod_fns) {
                match entry {
                    Some(current) if current <= callee.name.as_str() => {}
                    _ => entry = Some(callee.name.as_str()),
                }
            }
        }
        if let Some(symbol) = entry {
            witnesses.push((
                test.file.clone(),
                test.start_line,
                test.name.clone(),
                symbol.to_string(),
            ));
        }
    }

    if witnesses.is_empty() {
        return None;
    }
    witnesses.sort();
    let other_test_count = witnesses.len() - 1;
    let (test_file, test_line, test_name, entry_symbol) = witnesses.into_iter().next()?;
    Some(TransitiveWitness {
        test_name,
        test_file,
        test_line,
        entry_symbol,
        other_test_count,
    })
}

/// Builds the concrete witness pointer appended after
/// [`RUST_TRANSITIVE_REACH_MESSAGE`]. Names the witnessing test (file:line) and
/// the entry symbol, using candidate ("may lead here") language only - it never
/// claims the test reaches, covers, or exercises the change.
///
/// The rendered file path normalizes `\\` to `/` so Windows-blessed goldens
/// match on Linux CI.
pub(in crate::analysis) fn transitive_reach_witness_pointer(witness: &TransitiveWitness) -> String {
    let location = format!(
        "{}:{}",
        witness.test_file.display().to_string().replace('\\', "/"),
        witness.test_line
    );
    let others = match witness.other_test_count {
        0 => String::new(),
        1 => " (and 1 other test)".to_string(),
        n => format!(" (and {n} other tests)"),
    };
    format!(
        "{}`{}` ({}) calls `{}`, an entry point that may lead here{}. \
         Inspect it to judge whether this change is observed.",
        crate::domain::TRANSITIVE_REACH_WITNESS_PREFIX,
        witness.test_name,
        location,
        witness.entry_symbol,
        others
    )
}

/// Collect all tests from the index, deduplicating by (name, file).
fn collect_all_tests(index: &RustIndex) -> Vec<&TestFact> {
    let mut seen: HashSet<(&str, &std::path::Path)> = HashSet::new();
    let mut v: Vec<&TestFact> = Vec::new();
    for t in &index.tests {
        if seen.insert((t.name.as_str(), t.file.as_path())) {
            v.push(t);
        }
    }
    for file in index.files.values() {
        for t in &file.tests {
            if seen.insert((t.name.as_str(), t.file.as_path())) {
                v.push(t);
            }
        }
    }
    v
}

/// Find a production function by name (first match; lexical, no resolution).
fn find_prod_fn_by_name<'a>(
    name: &str,
    prod_fns: &[&'a FunctionSummary],
) -> Option<&'a FunctionSummary> {
    prod_fns.iter().find(|f| f.name == name).copied()
}

/// BFS from `start_name` through production function call facts to see if
/// `owner_name` is reachable within `MAX_TRANSITIVE_DEPTH` hops.
///
/// Stops early at any boundary:
/// - Callee name is a macro invocation (`name!`).
/// - Callee is not found in the production function set (external / unresolved).
/// - Depth exceeds `MAX_TRANSITIVE_DEPTH`.
fn bfs_reaches_owner(start_name: &str, owner_name: &str, prod_fns: &[&FunctionSummary]) -> bool {
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();

    queue.push_back((start_name.to_string(), 1));
    visited.insert(start_name.to_string());

    while let Some((current_name, depth)) = queue.pop_front() {
        if depth > MAX_TRANSITIVE_DEPTH {
            continue;
        }
        let Some(current_fn) = find_prod_fn_by_name(&current_name, prod_fns) else {
            // Callee not found in-crate - stop this branch (fail closed).
            continue;
        };
        for call in calls_of(current_fn) {
            // Stop at macro invocations.
            if is_macro_call(call.name.as_str()) {
                continue;
            }
            if call.name == owner_name {
                return true;
            }
            if !visited.contains(&call.name) {
                visited.insert(call.name.clone());
                queue.push_back((call.name.clone(), depth + 1));
            }
        }
    }

    false
}

fn calls_of(f: &FunctionSummary) -> &[CallFact] {
    &f.calls
}

/// Returns true when the callee name looks like a macro invocation - i.e. it
/// contains `!`. Lexical call extraction in ripr may or may not retain the
/// bang; we check containment to fail closed.
fn is_macro_call(name: &str) -> bool {
    name.contains('!')
}

/// The human/JSON message emitted as a stop-reason when the transitive
/// limitation fires. This is a named limitation, NOT a coverage claim.
pub(in crate::analysis) const RUST_TRANSITIVE_REACH_MESSAGE: &str = "ripr saw a test reaching public API that may call toward this change \
     through a transitive path it does not fully trace \
     (pub to pub(crate) helper chains, macros, or generics). \
     This is not a coverage assessment -- ripr cannot confirm or deny \
     that the change is observed.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::{CallFact, FileFacts, FunctionSummary, RustIndex, TestFact};
    use crate::domain::SymbolId;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn make_fn(name: &str, calls: Vec<&str>) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("src/lib.rs::{name}")),
            name: name.to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 10,
            body: String::new(),
            calls: calls
                .into_iter()
                .map(|c| CallFact {
                    line: 2,
                    name: c.to_string(),
                    text: format!("{c}()"),
                })
                .collect(),
            returns: Vec::new(),
            literals: Vec::new(),
            is_test: false,
            attrs: Vec::new(),
        }
    }

    fn make_test(name: &str, calls: Vec<&str>) -> TestFact {
        make_test_at(name, "tests/it.rs", 1, calls)
    }

    fn make_test_at(name: &str, file: &str, start_line: usize, calls: Vec<&str>) -> TestFact {
        TestFact {
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line,
            end_line: start_line + 4,
            body: String::new(),
            calls: calls
                .into_iter()
                .map(|c| CallFact {
                    line: 2,
                    name: c.to_string(),
                    text: format!("{c}()"),
                })
                .collect(),
            assertions: Vec::new(),
            literals: Vec::new(),
            attrs: Vec::new(),
        }
    }

    fn index_with(fns: Vec<FunctionSummary>, tests: Vec<TestFact>) -> RustIndex {
        let mut files: BTreeMap<std::path::PathBuf, FileFacts> = BTreeMap::new();
        let path = PathBuf::from("src/lib.rs");
        files.insert(
            path.clone(),
            FileFacts {
                path,
                functions: fns,
                tests: Vec::new(),
                calls: Vec::new(),
                returns: Vec::new(),
                literals: Vec::new(),
                probe_shapes: Vec::new(),
                source: String::new(),
            },
        );
        RustIndex {
            files,
            tests,
            functions: Vec::new(),
        }
    }

    // (a) Candidate path found -> witness captured naming the test + entry symbol.
    // test calls `outer`, `outer` calls `inner` (the changed owner).
    #[test]
    fn given_test_calls_outer_which_calls_owner_then_witness_is_captured() {
        let outer = make_fn("outer", vec!["inner"]);
        let index = index_with(
            vec![outer],
            vec![make_test("test_uses_outer", vec!["outer"])],
        );

        let witness = find_transitive_witness("inner", &index);
        assert_eq!(
            witness.as_ref().map(|w| w.test_name.as_str()),
            Some("test_uses_outer")
        );
        assert_eq!(
            witness.as_ref().map(|w| w.test_file.clone()),
            Some(PathBuf::from("tests/it.rs"))
        );
        assert_eq!(
            witness.as_ref().map(|w| w.entry_symbol.as_str()),
            Some("outer")
        );
        assert_eq!(witness.as_ref().map(|w| w.other_test_count), Some(0));
    }

    // (b) No path -> witness must be None.
    // test calls `unrelated`, no path to owner `inner`.
    #[test]
    fn given_no_path_to_owner_then_witness_is_none() {
        let unrelated = make_fn("unrelated", vec!["helper"]);
        let helper = make_fn("helper", vec![]);
        let index = index_with(
            vec![unrelated, helper],
            vec![make_test("test_unrelated", vec!["unrelated"])],
        );

        assert!(find_transitive_witness("inner", &index).is_none());
    }

    // (a') Two witnessing tests -> the first by (file, line, name) is named and
    // the count of others is reported. `tests/a.rs` sorts before `tests/b.rs`.
    #[test]
    fn given_two_witnesses_then_first_by_file_line_is_selected() {
        let outer = make_fn("outer", vec!["inner"]);
        let index = index_with(
            vec![outer],
            vec![
                make_test_at("test_b", "tests/b.rs", 1, vec!["outer"]),
                make_test_at("test_a", "tests/a.rs", 1, vec!["outer"]),
            ],
        );

        let witness = find_transitive_witness("inner", &index);
        assert_eq!(
            witness.as_ref().map(|w| w.test_file.clone()),
            Some(PathBuf::from("tests/a.rs"))
        );
        assert_eq!(
            witness.as_ref().map(|w| w.test_name.as_str()),
            Some("test_a")
        );
        assert_eq!(witness.as_ref().map(|w| w.other_test_count), Some(1));
    }

    // The witness pointer names the test/entry symbol with candidate language
    // only: it must say "may lead here" and must NOT claim the test reaches,
    // covers, or exercises the change.
    #[test]
    fn witness_pointer_uses_may_language_and_no_coverage_claim() {
        let witness = TransitiveWitness {
            test_name: "test_uses_outer".to_string(),
            test_file: PathBuf::from("tests/it.rs"),
            test_line: 12,
            entry_symbol: "outer".to_string(),
            other_test_count: 0,
        };
        let pointer = transitive_reach_witness_pointer(&witness);
        assert!(pointer.contains("test_uses_outer"));
        assert!(pointer.contains("tests/it.rs:12"));
        assert!(pointer.contains("outer"));
        assert!(pointer.contains("may lead here"));
        assert!(!pointer.contains("reaches"));
        assert!(!pointer.contains("covers"));
        assert!(!pointer.contains("exercise"));
    }

    // The rendered location normalizes backslashes so Windows-blessed goldens
    // match on Linux CI.
    #[test]
    fn witness_pointer_normalizes_backslashes_in_path() {
        let witness = TransitiveWitness {
            test_name: "t".to_string(),
            test_file: PathBuf::from("tests\\sub\\it.rs"),
            test_line: 3,
            entry_symbol: "outer".to_string(),
            other_test_count: 0,
        };
        let pointer = transitive_reach_witness_pointer(&witness);
        assert!(pointer.contains("tests/sub/it.rs:3"));
        assert!(!pointer.contains('\\'));
    }

    // Plural form when more than one other test witnesses.
    #[test]
    fn witness_pointer_reports_plural_other_tests() {
        let witness = TransitiveWitness {
            test_name: "t".to_string(),
            test_file: PathBuf::from("tests/it.rs"),
            test_line: 1,
            entry_symbol: "outer".to_string(),
            other_test_count: 2,
        };
        assert!(transitive_reach_witness_pointer(&witness).contains("and 2 other tests"));
    }

    // (c-i) Path exists at exactly depth=3 -> witness captured (boundary is depth > 3 not >= 3).
    // test -> fn_a(1) -> fn_b(2) -> fn_c(3) -> check fn_c.calls: includes inner.
    #[test]
    fn given_path_at_depth_3_then_witness_is_captured() {
        let fn_a = make_fn("fn_a", vec!["fn_b"]);
        let fn_b = make_fn("fn_b", vec!["fn_c"]);
        let fn_c = make_fn("fn_c", vec!["inner"]);
        let index = index_with(
            vec![fn_a, fn_b, fn_c],
            vec![make_test("test_depth3", vec!["fn_a"])],
        );
        // fn_a(1) -> fn_b(2) -> fn_c(3): at depth=3 we look at fn_c.calls -> inner.
        let witness = find_transitive_witness("inner", &index);
        assert_eq!(
            witness.as_ref().map(|w| w.entry_symbol.as_str()),
            Some("fn_a")
        );
    }

    // (c-ii) Path at depth=4 -> exceeds MAX_TRANSITIVE_DEPTH=3, NOT found.
    // fn_c is popped at depth=3, its calls include fn_d -> push fn_d at depth=4.
    // fn_d is popped at depth=4, 4 > 3 -> continue. inner NOT reached.
    #[test]
    fn given_path_depth_4_then_witness_is_none() {
        let fn_a = make_fn("fn_a", vec!["fn_b"]);
        let fn_b = make_fn("fn_b", vec!["fn_c"]);
        let fn_c = make_fn("fn_c", vec!["fn_d"]);
        let fn_d = make_fn("fn_d", vec!["inner"]);
        let index = index_with(
            vec![fn_a, fn_b, fn_c, fn_d],
            vec![make_test("test_too_deep", vec!["fn_a"])],
        );
        assert!(find_transitive_witness("inner", &index).is_none());
    }

    // (c-iii) Macro call in chain is skipped; no path found through a macro entry.
    #[test]
    fn given_macro_call_in_test_calls_then_witness_is_none() {
        // test calls only a macro -> no path found.
        let index = index_with(
            vec![make_fn("inner", vec![])],
            vec![make_test("test_macro_only", vec!["vec!"])],
        );
        assert!(find_transitive_witness("inner", &index).is_none());
    }

    // (c-iv) Callee not found in-crate -> walk stops there (fail closed).
    #[test]
    fn given_callee_not_in_crate_then_witness_is_none() {
        let outer = make_fn("outer", vec!["external_lib_helper"]);
        // external_lib_helper is NOT in production functions.
        let index = index_with(vec![outer], vec![make_test("test_ext", vec!["outer"])]);
        assert!(find_transitive_witness("inner", &index).is_none());
    }

    #[test]
    fn is_macro_call_detects_bang_in_name() {
        assert!(is_macro_call("vec!"));
        assert!(is_macro_call("format!"));
        assert!(is_macro_call("assert!"));
        assert!(!is_macro_call("outer"));
        assert!(!is_macro_call("inner"));
        assert!(!is_macro_call(""));
    }

    #[test]
    fn wire_message_contains_honest_may_language_and_no_coverage_claim() {
        assert!(RUST_TRANSITIVE_REACH_MESSAGE.contains("may"));
        assert!(RUST_TRANSITIVE_REACH_MESSAGE.contains("not a coverage assessment"));
        assert!(!RUST_TRANSITIVE_REACH_MESSAGE.contains("reaches the change"));
        assert!(!RUST_TRANSITIVE_REACH_MESSAGE.contains("covers"));
        assert!(!RUST_TRANSITIVE_REACH_MESSAGE.contains("tested"));
    }
}
