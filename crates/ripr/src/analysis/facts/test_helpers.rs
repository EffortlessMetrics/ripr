//! Same-file test-side assertion helpers.
//!
//! Rust suites often move the call and the assertion into a helper and keep
//! each test to a list of cases:
//!
//! ```text
//! fn check(input: u32, expected: bool) { assert_eq!(gate(input), expected); }
//!
//! #[test]
//! fn boundary() { check(10, false); check(11, true); }
//! ```
//!
//! Without this pass the test body has no owner call and no assertion, so a
//! changed `gate` reads as reached only by name and never discriminated
//! (`reachable_unrevealed`), although mutating `gate` fails the test.
//!
//! This pass is the one authority that credits such helpers. It runs after
//! every role authority, so the helper's role is final, and it widens a
//! test's facts in one bounded direction:
//!
//! - the helper is a non-test, evidence-role (`CfgTestModule`) function in
//!   the test's own file, its name is defined exactly once there, and it
//!   is a direct item of the same inline module as the test (or both sit at
//!   the file's top level). A module item cannot coexist with a same-named
//!   `use` import and wins over a glob, so the call resolves to it. A
//!   helper in a sibling or parent module (`use super::*`), or nested in
//!   another fn's body, is not credited, and neither is any helper for a
//!   test whose body holds a `use` item or any `cfg`/`cfg_attr` attribute;
//! - the parsed test body calls it as a single-segment free function
//!   (`check(..)`, not `self.check(..)`, `path::check(..)`, or the text
//!   `check(` in a string or comment), outside any closure, `async`
//!   block or nested `fn` it may never run, and the test binds no name equal
//!   to it anywhere (parameter, `let`, `for`, `if let`, closure or match
//!   binding, or any named item such as `const`, `static`, nested `fn` or
//!   tuple `struct`);
//! - the helper has no `cfg`/`cfg_attr` attribute anywhere (a disabled
//!   helper can stand beside a macro-defined real one), it is not `async`
//!   and holds no closure or `async` block (an assertion there may never
//!   run), no `use` item or nested `fn` in its body, binds no name it also calls (a `let`
//!   closure over the owner's name), and does not share a line with the
//!   test;
//! - one hop: the helper's calls and parser-backed assertions are added
//!   with the helper's own line numbers; the assertions of helpers the
//!   helper calls are not followed.
//!
//! The credited calls sit outside the test's line span. Value resolution
//! reads only [`TestFact::body_calls`], because a helper call's arguments
//! name the helper's parameters, which the test's `let` bindings and case
//! rows do not bind. Relation and reach read every call.
//!
//! Anything else (cross-file helpers, ambiguous names, production callees,
//! the lexical fallback) contributes nothing, so a test only ever gains
//! evidence the helper body really contains.

use super::{FunctionFact, FunctionSourceRole, RustIndex, TestFact};
use crate::analysis::syntax::{ModuleItemScopes, module_item_scopes, parser_oracles_for_function};
use rayon::prelude::*;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

pub(super) fn credit_same_file_assertion_helpers(index: &mut RustIndex) {
    // Only files that hold a test can credit a helper, so only those are
    // parsed, and the parses run on the rayon pool: every indexed file was
    // parsed serially here before, which dominated warm diff-scoped checks.
    let test_files: BTreeSet<&PathBuf> = index.tests().iter().map(|test| &test.file).collect();
    let files = index
        .files()
        .iter()
        .filter(|(file, facts)| !facts.used_lexical_fallback && test_files.contains(file))
        .collect::<Vec<_>>();
    let parsed = files
        .par_iter()
        .filter_map(|(file, facts)| Some((*file, *facts, module_item_scopes(&facts.source)?)))
        .collect::<Vec<_>>();
    let mut helpers_by_file: BTreeMap<PathBuf, BTreeMap<String, Vec<&FunctionFact>>> =
        BTreeMap::new();
    let mut scopes_by_file: BTreeMap<PathBuf, ModuleItemScopes> = BTreeMap::new();
    for (file, facts, scopes) in parsed {
        scopes_by_file.insert(file.clone(), scopes);
        let names = helpers_by_file.entry(file.clone()).or_default();
        for function in facts.functions.iter() {
            names
                .entry(function.name.clone())
                .or_default()
                .push(function);
        }
    }

    let mut widened: BTreeMap<(PathBuf, usize, String), TestFact> = BTreeMap::new();
    for test in index.tests().iter() {
        let (Some(functions_by_name), Some(scopes)) = (
            helpers_by_file.get(&test.file),
            scopes_by_file.get(&test.file),
        ) else {
            continue;
        };
        let test_key = (test.start_line, test.name.clone());
        // A `use` in the test body may import a same-named function over
        // the module's helper.
        // A cfg attribute in the test may disable the call being credited.
        if scopes.fns_with_local_use.contains(&test_key) || scopes.fns_with_cfg.contains(&test_key)
        {
            continue;
        }
        let Some(test_module) = scopes.item_fns.get(&test_key) else {
            continue;
        };
        let mut credited = test.clone();
        let mut credited_helpers: Vec<&str> = Vec::new();
        for call in &test.calls {
            if credited_helpers.contains(&call.name.as_str()) {
                continue;
            }
            let Some(helper) = unique_assertion_helper(functions_by_name, &call.name) else {
                continue;
            };
            let helper_key = (helper.start_line, helper.name.clone());
            if scopes.item_fns.get(&helper_key) != Some(test_module)
                || !calls_directly(scopes, &test_key, &call.name)
                || test_shadows(test, &call.name)
                || scopes
                    .bound_names
                    .get(&test_key)
                    .is_none_or(|bound| bound.contains(&call.name))
                || !helper_body_is_plain(scopes, &helper_key, helper)
                || spans_overlap(helper, test)
            {
                continue;
            }
            // Parser-backed oracles only, as for ordinary tests: a
            // commented-out `assert_eq!` must not count.
            let Some(assertions) = parser_oracles_for_function(&helper.body, helper.start_line)
            else {
                continue;
            };
            credited_helpers.push(&helper.name);
            credited.calls.extend(helper.calls.iter().cloned());
            credited.assertions.extend(assertions);
        }
        if !credited_helpers.is_empty() {
            widened.insert(
                (test.file.clone(), test.start_line, test.name.clone()),
                credited,
            );
        }
    }
    if widened.is_empty() {
        return;
    }

    // One write per stored record reaches both the flat and per-file views.
    index.for_each_test_mut(|test| {
        if let Some(credited) =
            widened.get(&(test.file.clone(), test.start_line, test.name.clone()))
        {
            *test = credited.clone();
        }
    });
}

/// The one same-file definition of `name`, when it is an evidence-only
/// helper. A second definition (a same-named test, production function or
/// helper in another module of the file) makes the call ambiguous.
fn unique_assertion_helper<'facts>(
    functions_by_name: &BTreeMap<String, Vec<&'facts FunctionFact>>,
    name: &str,
) -> Option<&'facts FunctionFact> {
    match functions_by_name.get(name)?.as_slice() {
        [helper] if helper.source_role == FunctionSourceRole::CfgTestModule => Some(helper),
        _ => None,
    }
}

/// A nested `fn` item or a `let` binding with the helper's name means the
/// call may not reach the file-level helper.
fn test_shadows(test: &TestFact, name: &str) -> bool {
    test.nested_fn_names.iter().any(|nested| nested == name)
        || test.let_bindings.iter().any(|binding| binding.name == name)
}

/// Whether the parsed test body calls `name` as a single-segment free
/// function: not a method, a path call, or text in a string or comment.
fn calls_directly(scopes: &ModuleItemScopes, test_key: &(usize, String), name: &str) -> bool {
    scopes
        .direct_calls
        .get(test_key)
        .is_some_and(|called| called.contains(name))
}

/// A helper whose assertion is the one that runs: no `cfg`/`cfg_attr`
/// attribute anywhere in it (a cfg-disabled helper can stand beside a
/// macro-defined real one), not `async` and no closure or `async` block in
/// its body (their assertions may never run), no `use` item or nested `fn`
/// in its body, and
/// no name it binds (a `let` closure, a parameter) that it also calls,
/// since any of these can shadow the owner the credited call names.
fn helper_body_is_plain(
    scopes: &ModuleItemScopes,
    helper_key: &(usize, String),
    helper: &FunctionFact,
) -> bool {
    !scopes.fns_with_cfg.contains(helper_key)
        && !scopes.fns_with_local_use.contains(helper_key)
        && !scopes.fns_with_deferred_code.contains(helper_key)
        && helper.nested_fn_names.is_empty()
        && scopes
            .bound_names
            .get(helper_key)
            .is_some_and(|bound| !helper.calls.iter().any(|call| bound.contains(&call.name)))
}

/// Credited calls must sit outside the test's line span, which is what
/// [`TestFact::body_calls`] relies on; a helper sharing a line with the
/// test is not credited.
fn spans_overlap(helper: &FunctionFact, test: &TestFact) -> bool {
    helper.start_line <= test.end_line && test.start_line <= helper.end_line
}

#[cfg(test)]
mod tests;
