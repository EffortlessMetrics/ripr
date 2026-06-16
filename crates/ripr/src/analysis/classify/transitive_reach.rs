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

/// Maximum call-hop depth for the transitive walk.
const MAX_TRANSITIVE_DEPTH: usize = 3;

/// Determines whether any test in the index may transitively reach a function
/// named `owner_name` via a bounded BFS over lexical call facts.
///
/// Returns `true` when a candidate path (test -> ... -> owner) is found within
/// `MAX_TRANSITIVE_DEPTH` hops using same-crate production functions only.
/// Returns `false` when no such candidate path exists.
///
/// The caller is responsible for wiring the result into `static_limit_kind`
/// ONLY when the finding's class is `no_static_path` and `related_tests` is
/// empty - i.e. only after the direct-call classifier has already returned
/// empty-handed.
pub(in crate::analysis) fn has_transitive_candidate(owner_name: &str, index: &RustIndex) -> bool {
    if owner_name.is_empty() {
        return false;
    }

    // Collect all tests from the index (flat tests vec + per-file tests).
    let all_tests = collect_all_tests(index);

    // Build a flat list of production (non-test) function facts for name lookup.
    let prod_fns: Vec<&FunctionSummary> = index
        .files
        .values()
        .flat_map(|file| file.functions.iter().filter(|f| !f.is_test))
        .collect();

    for test in &all_tests {
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
                return true;
            }
        }
    }

    false
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
        TestFact {
            name: name.to_string(),
            file: PathBuf::from("tests/it.rs"),
            start_line: 1,
            end_line: 5,
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

    // (a) Candidate path found -> limitation should fire.
    // test calls `outer`, `outer` calls `inner` (the changed owner).
    #[test]
    fn given_test_calls_outer_which_calls_owner_then_transitive_candidate_found() {
        let outer = make_fn("outer", vec!["inner"]);
        let index = index_with(
            vec![outer],
            vec![make_test("test_uses_outer", vec!["outer"])],
        );

        assert!(has_transitive_candidate("inner", &index));
    }

    // (b) No path -> limitation must NOT fire.
    // test calls `unrelated`, no path to owner `inner`.
    #[test]
    fn given_no_path_to_owner_then_no_transitive_candidate() {
        let unrelated = make_fn("unrelated", vec!["helper"]);
        let helper = make_fn("helper", vec![]);
        let index = index_with(
            vec![unrelated, helper],
            vec![make_test("test_unrelated", vec!["unrelated"])],
        );

        assert!(!has_transitive_candidate("inner", &index));
    }

    // (c-i) Path exists at exactly depth=3 -> candidate IS found (boundary is depth > 3 not >= 3).
    // test -> fn_a(1) -> fn_b(2) -> fn_c(3) -> check fn_c.calls: includes inner.
    // At depth=3 we pop fn_c, iterate its calls: inner == owner_name -> true.
    #[test]
    fn given_path_at_depth_3_then_transitive_candidate_found() {
        let fn_a = make_fn("fn_a", vec!["fn_b"]);
        let fn_b = make_fn("fn_b", vec!["fn_c"]);
        let fn_c = make_fn("fn_c", vec!["inner"]);
        let index = index_with(
            vec![fn_a, fn_b, fn_c],
            vec![make_test("test_depth3", vec!["fn_a"])],
        );
        // fn_a(1) -> fn_b(2) -> fn_c(3): at depth=3 we look at fn_c.calls -> inner.
        assert!(has_transitive_candidate("inner", &index));
    }

    // (c-ii) Path at depth=4 -> exceeds MAX_TRANSITIVE_DEPTH=3, NOT found.
    // test -> fn_a(1) -> fn_b(2) -> fn_c(3) -> fn_d(4) -> inner at depth 4 is skipped.
    // Wait: depth=4 means we pop fn_d at depth=4. 4 > 3 -> continue (skip). inner NOT found.
    // Actually: fn_c is popped at depth=3, its calls include fn_d -> push fn_d at depth=4.
    // fn_d is popped at depth=4, 4 > 3 -> continue. inner NOT reached.
    #[test]
    fn given_path_depth_4_then_no_transitive_candidate() {
        let fn_a = make_fn("fn_a", vec!["fn_b"]);
        let fn_b = make_fn("fn_b", vec!["fn_c"]);
        let fn_c = make_fn("fn_c", vec!["fn_d"]);
        let fn_d = make_fn("fn_d", vec!["inner"]);
        let index = index_with(
            vec![fn_a, fn_b, fn_c, fn_d],
            vec![make_test("test_too_deep", vec!["fn_a"])],
        );
        assert!(!has_transitive_candidate("inner", &index));
    }

    // (c-iii) Macro call in chain is skipped; other paths still work.
    #[test]
    fn given_macro_call_in_test_calls_then_macro_is_skipped() {
        // test calls only a macro -> no path found.
        let index = index_with(
            vec![make_fn("inner", vec![])],
            vec![make_test("test_macro_only", vec!["vec!"])],
        );
        assert!(!has_transitive_candidate("inner", &index));
    }

    // (c-iv) Callee not found in-crate -> walk stops there (fail closed).
    #[test]
    fn given_callee_not_in_crate_then_walk_stops_fail_closed() {
        let outer = make_fn("outer", vec!["external_lib_helper"]);
        // external_lib_helper is NOT in production functions.
        let index = index_with(vec![outer], vec![make_test("test_ext", vec!["outer"])]);
        assert!(!has_transitive_candidate("inner", &index));
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
