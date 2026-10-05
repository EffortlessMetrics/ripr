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
//! - Depth is bounded at 5 hops (`MAX_TRANSITIVE_DEPTH`).
//! - The walk stops (and NAMES the limitation) at any boundary:
//!   macro invocations (`name!`), callee names not found in the production
//!   function set, or depth > 5.
//! - Finding classification NEVER changes: `no_static_path` stays `no_static_path`.
//!   This check only sets `static_limit_kind` to name the limitation.
//! - If no candidate transitive path is found the finding is left exactly as-is.

use crate::analysis::facts::{CallFact, FunctionSummary, RustIndex, TestFact};
use crate::domain::StaticLimitKind;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock, PoisonError};

/// Maximum call-hop depth for the transitive walk.
pub(in crate::analysis) const MAX_TRANSITIVE_DEPTH: usize = 5;

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

/// A concrete pointer to a macro-blocked Rust reach candidate.
///
/// This witness is intentionally weaker than [`TransitiveWitness`]: it says a
/// test calls an entry symbol that reaches a same-repo macro invocation whose
/// definition lexically mentions the changed owner. ripr does not expand the
/// macro and does not add the test to `related_tests`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::analysis) struct MacroReachWitness {
    pub test_name: String,
    pub test_file: PathBuf,
    pub test_line: usize,
    pub entry_symbol: String,
    pub macro_name: String,
    pub macro_file: PathBuf,
    pub macro_line: usize,
    pub macro_host: String,
    pub other_test_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MacroInvocation {
    name: String,
    line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MacroReachEdge {
    macro_name: String,
    macro_file: PathBuf,
    macro_line: usize,
    macro_host: String,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MacroWitnessCandidate {
    test_file: PathBuf,
    test_line: usize,
    test_name: String,
    entry_symbol: String,
    macro_name: String,
    macro_file: PathBuf,
    macro_line: usize,
    macro_host: String,
}

pub(in crate::analysis) const MACRO_WITNESS_TEST_BODY_HOST: &str = "test body";

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
#[cfg(test)]
fn find_transitive_witness(owner_name: &str, index: &RustIndex) -> Option<TransitiveWitness> {
    TransitiveReachIndex::new(index).transitive_witness(owner_name)
}

/// Per-index reach facts shared by every `no_static_path` finding in one run.
///
/// The test list, the name-keyed production function index and the reverse
/// call graph depend only on the index, so they are built once (on first use)
/// instead of once per finding. Each finding then runs one reverse sweep from
/// its owner instead of a forward sweep from every test callee.
pub(in crate::analysis) struct TransitiveReachIndex<'a> {
    index: &'a RustIndex,
    graph: OnceLock<ReachGraph<'a>>,
}

struct ReachGraph<'a> {
    all_tests: Vec<&'a TestFact>,
    /// Each test body with comments and strings masked, aligned with
    /// `all_tests` and filled on first use. Witness corroboration reads a
    /// test's body for every reaching callee of every owner, and masking it
    /// each time made rust-analyzer's cold pilot 3.5x slower (#6009).
    masked_test_bodies: Vec<OnceLock<String>>,
    /// Per test: `(method, impl type)` to whether the test calls the method
    /// on that type. The answer depends only on the test, and every owner
    /// whose reaching set holds the method asks again.
    receiver_checks: Vec<Mutex<HashMap<(String, String), bool>>>,
    /// Every production function with a given name. Name-only facts cannot
    /// tell `StringDecoder::decode` from `StringDecoderRange::decode`, so the
    /// walk follows all of them rather than whichever one was indexed first.
    by_name: HashMap<&'a str, Vec<&'a FunctionSummary>>,
    /// Callee name to the distinct production function names that call it,
    /// over non-macro call facts: the forward walk's edges, reversed.
    callers: HashMap<&'a str, Vec<&'a str>>,
    /// Body of every `macro_rules!` definition in the index. A macro edge
    /// needs a definition whose body names the owner, so an owner no body
    /// names cannot have one.
    macro_bodies: Vec<&'a str>,
    /// Built on the first macro sweep; see [`Self::macro_invocations`].
    macro_invocations: OnceLock<MacroInvocations<'a>>,
    /// Per macro name: one entry per `macro_rules!` definition of that name,
    /// holding its body when it has one, as [`macro_definitions_named`]
    /// would find them. Built in one pass over every source, instead of one
    /// pass per owner and invoked macro name.
    macro_definitions: HashMap<&'a str, Vec<Option<&'a str>>>,
}

struct MacroInvocations<'a> {
    /// Per test, in `ReachGraph::all_tests` order.
    tests: Vec<Vec<MacroInvocation>>,
    /// Per production function, aligned with `ReachGraph::by_name`.
    by_name: HashMap<&'a str, Vec<Vec<MacroInvocation>>>,
}

impl<'a> ReachGraph<'a> {
    fn build(index: &'a RustIndex) -> Self {
        let all_tests = collect_all_tests(index);
        let mut by_name: HashMap<&'a str, Vec<&'a FunctionSummary>> = HashMap::new();
        let mut callers: HashMap<&'a str, Vec<&'a str>> = HashMap::new();
        for function in index.files().values().flat_map(|file| {
            file.functions
                .iter()
                .filter(|f| !f.source_role.is_evidence_role())
        }) {
            by_name
                .entry(function.name.as_str())
                .or_default()
                .push(function);
            for call in calls_of(function) {
                if !is_macro_call(call.name.as_str()) {
                    callers
                        .entry(call.name.as_str())
                        .or_default()
                        .push(function.name.as_str());
                }
            }
        }
        for names in callers.values_mut() {
            names.sort_unstable();
            names.dedup();
        }
        let mut macro_bodies = Vec::new();
        let mut macro_definitions: HashMap<&'a str, Vec<Option<&'a str>>> = HashMap::new();
        for file in index.files().values() {
            add_macro_definitions(
                &file.data().source,
                &mut macro_bodies,
                &mut macro_definitions,
            );
        }
        let masked_test_bodies = all_tests.iter().map(|_| OnceLock::new()).collect();
        let receiver_checks = all_tests.iter().map(|_| Mutex::default()).collect();
        Self {
            all_tests,
            masked_test_bodies,
            receiver_checks,
            by_name,
            callers,
            macro_bodies,
            macro_invocations: OnceLock::new(),
            macro_definitions,
        }
    }

    /// Macro invocations in every test and production function body. Every
    /// macro sweep reads them and they depend only on the bodies, so they are
    /// built once, on the first sweep; a run that never reaches the macro
    /// fallback does not pay for them.
    fn macro_invocations(&self) -> &MacroInvocations<'a> {
        self.macro_invocations.get_or_init(|| MacroInvocations {
            tests: self
                .all_tests
                .iter()
                .map(|test| macro_invocations_in_text(&test.body, test.start_line))
                .collect(),
            by_name: self
                .by_name
                .iter()
                .map(|(&name, functions)| {
                    let invocations = functions
                        .iter()
                        .map(|function| {
                            macro_invocations_in_text(&function.body, function.start_line)
                        })
                        .collect();
                    (name, invocations)
                })
                .collect(),
        })
    }

    /// Whether `macro_name` has exactly one `macro_rules!` definition in the
    /// index and its body names `owner_name` as an identifier.
    fn macro_definition_mentions_owner(&self, macro_name: &str, owner_name: &str) -> bool {
        matches!(
            self.macro_definitions.get(macro_name).map(Vec::as_slice),
            Some([Some(body)]) if contains_identifier(body, owner_name)
        )
    }

    /// Names from which the forward walk reaches `owner_name`: every name
    /// whose shortest non-macro call chain to the owner, through production
    /// functions resolved by name, has 1 to `MAX_TRANSITIVE_DEPTH` hops.
    ///
    /// The forward walk starts at depth 1, expands names up to that depth,
    /// and succeeds when an expanded name calls the owner, so it succeeds
    /// exactly when such a chain exists. Walking the reversed edges from the
    /// owner finds the same set once per owner.
    fn names_reaching(&self, owner_name: &str) -> HashSet<&'a str> {
        let mut reaching: HashSet<&'a str> = HashSet::new();
        let mut frontier: Vec<&str> = vec![owner_name];
        let mut seen: HashSet<&str> = HashSet::from([owner_name]);
        for _ in 0..MAX_TRANSITIVE_DEPTH {
            let mut next = Vec::new();
            for callee in frontier {
                for caller in self.callers.get(callee).into_iter().flatten() {
                    if seen.insert(caller) {
                        reaching.insert(caller);
                        next.push(*caller);
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            frontier = next;
        }
        reaching
    }

    /// Whether a test gives a name-only reason to believe its call to
    /// `entry` lands on a function that reaches the owner, rather than on an
    /// unrelated function that shares the name (#5481: a unit test calling
    /// `Cache::build` while the path runs through `Site::build`).
    ///
    /// The entry is corroborated when some production function named `entry`
    /// that calls the owner or a reaching name is a free function, or is an
    /// associated function the test calls on a receiver resolved to its
    /// `impl` self type (constructor, annotation, UFCS or struct literal; see
    /// `method_call_resolves_to_impl_type`, which fails closed on an
    /// unresolved receiver). When no such function is found (an index without
    /// impl segments, or a depth edge), the entry counts as corroborated,
    /// which keeps the plain file-order selection. This only ranks witnesses;
    /// it never removes one or changes the classification.
    ///
    /// The entry is corroborated for a test exactly when this returns `None`
    /// or the test calls the entry on one of the returned self types
    /// (`test_calls_on_type`). Only the second half depends on the test, so
    /// `transitive_witness` resolves this once per entry.
    fn entry_receiver_types(
        &self,
        entry: &str,
        reaching: &HashSet<&str>,
        owner_name: &str,
    ) -> Option<Vec<String>> {
        let mut self_types = Vec::new();
        for function in self.by_name.get(entry).into_iter().flatten() {
            let reaches = calls_of(function).iter().any(|call| {
                !is_macro_call(&call.name)
                    && !is_own_declaration(call, function)
                    && (call.name == owner_name || reaching.contains(call.name.as_str()))
            });
            if !reaches {
                continue;
            }
            self_types.push(super::related_tests::impl_self_type_name(&function.id.0)?);
        }
        if self_types.is_empty() {
            None
        } else {
            Some(self_types)
        }
    }

    /// Whether test `test_index` calls `method` on a receiver resolved to
    /// `impl_type`; see `method_call_resolves_to_impl_type`.
    fn test_calls_on_type(&self, test_index: usize, method: &str, impl_type: &str) -> bool {
        let (Some(test), Some(masked), Some(checks)) = (
            self.all_tests.get(test_index),
            self.masked_test_bodies.get(test_index),
            self.receiver_checks.get(test_index),
        ) else {
            return false;
        };
        let key = (method.to_string(), impl_type.to_string());
        let lock = || checks.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(&known) = lock().get(&key) {
            return known;
        }
        let masked_body =
            masked.get_or_init(|| crate::analysis::extract::mask_comments_and_strings(&test.body));
        let calls = super::related_tests::method_call_resolves_to_impl_type_in(
            test,
            masked_body,
            method,
            impl_type,
        );
        lock().insert(key, calls);
        calls
    }
}

impl<'a> TransitiveReachIndex<'a> {
    pub(in crate::analysis) fn new(index: &'a RustIndex) -> Self {
        Self {
            index,
            graph: OnceLock::new(),
        }
    }

    fn graph(&self) -> &ReachGraph<'a> {
        self.graph.get_or_init(|| ReachGraph::build(self.index))
    }

    /// The bounded transitive witness for `owner_name`; see the module docs.
    pub(in crate::analysis) fn transitive_witness(
        &self,
        owner_name: &str,
    ) -> Option<TransitiveWitness> {
        if owner_name.is_empty() {
            return None;
        }
        let graph = self.graph();
        let reaching = graph.names_reaching(owner_name);
        if reaching.is_empty() {
            return None;
        }

        // One witness per test: its best entry symbol that reaches the owner,
        // where an entry the test body corroborates (see
        // `ReachGraph::entry_receiver_types`) beats a bare name match, then
        // the lexicographically-smallest name. Collected as a sortable tuple
        // with the corroboration rank first, so a test calling an unrelated
        // type's same-named method cannot win on file order alone (#5481),
        // and the named witness stays stable across index iteration order
        // (goldens depend on this determinism).
        let mut witnesses: Vec<(bool, PathBuf, usize, String, String)> = Vec::new();
        // The receiver types depend only on the entry and this owner, so
        // they are resolved once per entry rather than once per test.
        let mut receiver_types: HashMap<&str, Option<Vec<String>>> = HashMap::new();
        for (test_index, test) in graph.all_tests.iter().enumerate() {
            let mut entry: Option<(bool, &str)> = None;
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
                if reaching.contains(callee.name.as_str()) {
                    let name = callee.name.as_str();
                    let self_types = receiver_types
                        .entry(name)
                        .or_insert_with(|| graph.entry_receiver_types(name, &reaching, owner_name));
                    let uncorroborated = self_types.as_ref().is_some_and(|self_types| {
                        !self_types
                            .iter()
                            .any(|self_type| graph.test_calls_on_type(test_index, name, self_type))
                    });
                    let candidate = (uncorroborated, callee.name.as_str());
                    match entry {
                        Some(current) if current <= candidate => {}
                        _ => entry = Some(candidate),
                    }
                }
            }
            if let Some((uncorroborated, symbol)) = entry {
                witnesses.push((
                    uncorroborated,
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
        let (_, test_file, test_line, test_name, entry_symbol) = witnesses.into_iter().next()?;
        Some(TransitiveWitness {
            test_name,
            test_file,
            test_line,
            entry_symbol,
            other_test_count,
        })
    }
}

impl<'a> TransitiveReachIndex<'a> {
    /// Production functions a test may run: every non-macro name a test
    /// calls, and every production function those reach in at most
    /// `MAX_TRANSITIVE_DEPTH - 1` further hops, matched by name the same way
    /// as [`Self::transitive_witness`]. Pilot's trait-dispatch reach check
    /// (#5411) reads it to ask whether any test-reached code names a type.
    pub(in crate::analysis) fn test_reached_functions(&self) -> Vec<&'a FunctionSummary> {
        let graph = self.graph();
        let calls = graph.all_tests.iter().flat_map(|test| test.calls.iter());
        self.functions_reached_from(calls, MAX_TRANSITIVE_DEPTH)
    }

    /// Production functions `calls` may run within `depth` hops, matched by
    /// name, sorted by name. Macro invocations and names with no in-crate
    /// function stop their branch, as in the transitive walk.
    pub(in crate::analysis) fn functions_reached_from<'c>(
        &self,
        calls: impl IntoIterator<Item = &'c CallFact>,
        depth: usize,
    ) -> Vec<&'a FunctionSummary> {
        let graph = self.graph();
        let mut seen: HashSet<&'a str> = HashSet::new();
        let mut frontier: Vec<&'a str> = Vec::new();
        for call in calls {
            if is_macro_call(&call.name) {
                continue;
            }
            if let Some((&name, _)) = graph.by_name.get_key_value(call.name.as_str())
                && seen.insert(name)
            {
                frontier.push(name);
            }
        }
        let mut reached: Vec<&'a str> = frontier.clone();
        for _ in 1..depth {
            let mut next = Vec::new();
            for name in frontier {
                for function in graph.by_name.get(name).into_iter().flatten() {
                    for call in calls_of(function) {
                        if is_macro_call(&call.name) {
                            continue;
                        }
                        if let Some((&callee, _)) = graph.by_name.get_key_value(call.name.as_str())
                            && seen.insert(callee)
                        {
                            next.push(callee);
                        }
                    }
                }
            }
            if next.is_empty() {
                break;
            }
            reached.extend(next.iter().copied());
            frontier = next;
        }
        reached.sort_unstable();
        reached
            .into_iter()
            .flat_map(|name| graph.by_name.get(name).into_iter().flatten().copied())
            .collect()
    }

    /// Every production function, for checks that scan impl owners.
    pub(in crate::analysis) fn production_functions(
        &self,
    ) -> impl Iterator<Item = &'a FunctionSummary> + '_ {
        self.graph().by_name.values().flatten().copied()
    }
}

/// Finds a deterministic macro-blocked witness for a `no_static_path` Rust
/// finding after the direct and bounded transitive checks found no confirmed
/// lexical path.
///
/// The witness only fires when a same-repo `macro_rules!` definition lexically
/// mentions the changed owner. This names the unresolved macro edge without
/// expanding it and without changing classification.
#[cfg(test)]
fn find_macro_reach_witness(owner_name: &str, index: &RustIndex) -> Option<MacroReachWitness> {
    TransitiveReachIndex::new(index).macro_reach_witness(owner_name)
}

impl TransitiveReachIndex<'_> {
    /// The macro-blocked reach witness for `owner_name`, tried after the
    /// transitive witness finds no lexical path.
    pub(in crate::analysis) fn macro_reach_witness(
        &self,
        owner_name: &str,
    ) -> Option<MacroReachWitness> {
        if owner_name.is_empty() {
            return None;
        }
        let graph = self.graph();
        if !graph
            .macro_bodies
            .iter()
            .any(|body| contains_identifier(body, owner_name))
        {
            return None;
        }
        let mut sweep = ReachSweep::new(graph, owner_name);
        macro_reach_witness_with(graph, &mut sweep, owner_name)
    }
}

fn macro_reach_witness_with(
    graph: &ReachGraph<'_>,
    sweep: &mut ReachSweep<'_, '_>,
    owner_name: &str,
) -> Option<MacroReachWitness> {
    let mut witnesses: Vec<MacroWitnessCandidate> = Vec::new();
    for (test, invocations) in graph.all_tests.iter().zip(&graph.macro_invocations().tests) {
        let mut found: Vec<(String, MacroReachEdge)> = Vec::new();

        for macro_invocation in invocations {
            if let Some(edge) = ReachSweep::macro_edge_for_invocation(
                &mut sweep.macro_mention_memo,
                macro_invocation,
                &test.file,
                MACRO_WITNESS_TEST_BODY_HOST,
                owner_name,
                graph,
            ) {
                found.push((format!("{}!", macro_invocation.name), edge));
            }
        }

        for callee in &test.calls {
            if is_macro_call(&callee.name) || callee.name == owner_name {
                continue;
            }
            if let Some(edge) = sweep.macro_edge(&callee.name) {
                found.push((callee.name.clone(), edge));
            }
        }

        found.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then(left.1.macro_file.cmp(&right.1.macro_file))
                .then(left.1.macro_line.cmp(&right.1.macro_line))
                .then(left.1.macro_name.cmp(&right.1.macro_name))
        });
        if let Some((entry_symbol, edge)) = found.into_iter().next() {
            witnesses.push(MacroWitnessCandidate {
                test_file: test.file.clone(),
                test_line: test.start_line,
                test_name: test.name.clone(),
                entry_symbol,
                macro_name: edge.macro_name,
                macro_file: edge.macro_file,
                macro_line: edge.macro_line,
                macro_host: edge.macro_host,
            });
        }
    }

    if witnesses.is_empty() {
        return None;
    }
    witnesses.sort();
    let other_test_count = witnesses.len() - 1;
    let candidate = witnesses.into_iter().next()?;

    Some(MacroReachWitness {
        test_name: candidate.test_name,
        test_file: candidate.test_file,
        test_line: candidate.test_line,
        entry_symbol: candidate.entry_symbol,
        macro_name: candidate.macro_name,
        macro_file: candidate.macro_file,
        macro_line: candidate.macro_line,
        macro_host: candidate.macro_host,
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

pub(in crate::analysis) fn transitive_reach_limitation_detail_lines(
    witness: &TransitiveWitness,
    owner_name: &str,
) -> [String; 4] {
    let location = format!(
        "{}:{}",
        witness.test_file.display().to_string().replace('\\', "/"),
        witness.test_line
    );
    [
        format!(
            "{}test `{}` ({}) -> entry `{}`",
            crate::domain::LIMITATION_LAST_ESTABLISHED_EDGE_PREFIX,
            witness.test_name,
            location,
            witness.entry_symbol
        ),
        format!(
            "{}entry `{}` -> owner `{}` through a transitive Rust helper path",
            crate::domain::LIMITATION_FIRST_UNRESOLVED_EDGE_PREFIX,
            witness.entry_symbol,
            owner_name
        ),
        format!(
            "{}analysis/rust-public-api-transitive-reach",
            crate::domain::LIMITATION_ANALYZER_ROUTE_PREFIX
        ),
        format!(
            "{}named limitation only; ripr cannot confirm or deny that this path observes the change",
            crate::domain::LIMITATION_NON_CLAIM_PREFIX
        ),
    ]
}

/// Builds the concrete macro witness pointer appended after
/// [`RUST_MACRO_REACH_MESSAGE`]. The pointer names the test, entry symbol, and
/// macro boundary, using "may" language only.
pub(in crate::analysis) fn macro_reach_witness_pointer(witness: &MacroReachWitness) -> String {
    let test_location = format!(
        "{}:{}",
        witness.test_file.display().to_string().replace('\\', "/"),
        witness.test_line
    );
    let macro_location = format!(
        "{}:{}",
        witness.macro_file.display().to_string().replace('\\', "/"),
        witness.macro_line
    );
    let others = match witness.other_test_count {
        0 => String::new(),
        1 => " (and 1 other test)".to_string(),
        n => format!(" (and {n} other tests)"),
    };
    if witness.macro_host == MACRO_WITNESS_TEST_BODY_HOST {
        return format!(
            "{}`{}` ({}) directly invokes macro `{}!` at {} whose definition \
             lexically mentions the changed owner name. The macro path may \
             lead here{}. Inspect it to judge whether this change is observed.",
            crate::domain::TRANSITIVE_REACH_WITNESS_PREFIX,
            witness.test_name,
            test_location,
            witness.macro_name,
            macro_location,
            others
        );
    }
    format!(
        "{}`{}` ({}) calls `{}`, and `{}` invokes macro `{}!` at {} whose \
         definition lexically mentions the changed owner name. The macro path \
         may lead here{}. Inspect it to judge whether this change is observed.",
        crate::domain::TRANSITIVE_REACH_WITNESS_PREFIX,
        witness.test_name,
        test_location,
        witness.entry_symbol,
        witness.macro_host,
        witness.macro_name,
        macro_location,
        others
    )
}

pub(in crate::analysis) fn macro_reach_limitation_detail_lines(
    witness: &MacroReachWitness,
    owner_name: &str,
) -> [String; 4] {
    let test_location = format!(
        "{}:{}",
        witness.test_file.display().to_string().replace('\\', "/"),
        witness.test_line
    );
    let macro_location = format!(
        "{}:{}",
        witness.macro_file.display().to_string().replace('\\', "/"),
        witness.macro_line
    );
    let last_established_edge = if witness.macro_host == MACRO_WITNESS_TEST_BODY_HOST {
        format!(
            "{}test `{}` ({}) -> direct macro `{}!` at {}",
            crate::domain::LIMITATION_LAST_ESTABLISHED_EDGE_PREFIX,
            witness.test_name,
            test_location,
            witness.macro_name,
            macro_location
        )
    } else {
        format!(
            "{}test `{}` ({}) -> entry `{}` -> macro `{}!` at {}",
            crate::domain::LIMITATION_LAST_ESTABLISHED_EDGE_PREFIX,
            witness.test_name,
            test_location,
            witness.entry_symbol,
            witness.macro_name,
            macro_location
        )
    };
    [
        last_established_edge,
        format!(
            "{}macro `{}!` expansion toward owner `{}`",
            crate::domain::LIMITATION_FIRST_UNRESOLVED_EDGE_PREFIX,
            witness.macro_name,
            owner_name
        ),
        format!(
            "{}analysis/rust-macro-aware-reach",
            crate::domain::LIMITATION_ANALYZER_ROUTE_PREFIX
        ),
        format!(
            "{}named limitation only; ripr cannot confirm or deny that the macro-generated path observes the change",
            crate::domain::LIMITATION_NON_CLAIM_PREFIX
        ),
    ]
}

/// Collect all tests from the index, deduplicating by (name, file).
fn collect_all_tests(index: &RustIndex) -> Vec<&TestFact> {
    let mut seen: HashSet<(&str, &std::path::Path)> = HashSet::new();
    let mut v: Vec<&TestFact> = Vec::new();
    for t in &index.tests() {
        if seen.insert((t.name.as_str(), t.file.as_path())) {
            v.push(t);
        }
    }
    for file in index.files().values() {
        for t in &file.tests {
            if seen.insert((t.name.as_str(), t.file.as_path())) {
                v.push(t);
            }
        }
    }
    v
}

/// One owner's macro-reach sweeps over the shared name-keyed function index.
///
/// The sweeps are pure functions of (start name, owner name, index), so
/// per-start result memos collapse the repeated test × callee × BFS rescans
/// without changing any traversal order, first-match resolution, or witness
/// selection (goldens depend on all three).
struct ReachSweep<'g, 'a> {
    graph: &'g ReachGraph<'a>,
    owner_name: String,
    macro_edge_memo: HashMap<String, Option<MacroReachEdge>>,
    macro_mention_memo: HashMap<String, bool>,
}

impl<'g, 'a> ReachSweep<'g, 'a> {
    fn new(graph: &'g ReachGraph<'a>, owner_name: &str) -> Self {
        Self {
            graph,
            owner_name: owner_name.to_string(),
            macro_edge_memo: HashMap::new(),
            macro_mention_memo: HashMap::new(),
        }
    }

    fn resolve(&self, name: &str) -> &'g [&'a FunctionSummary] {
        self.graph.by_name.get(name).map_or(&[], Vec::as_slice)
    }

    fn macro_edge(&mut self, start_name: &str) -> Option<MacroReachEdge> {
        if let Some(cached) = self.macro_edge_memo.get(start_name) {
            return cached.clone();
        }
        let edge = self.bfs_hits_owner_macro_uncached(start_name);
        self.macro_edge_memo
            .insert(start_name.to_string(), edge.clone());
        edge
    }

    fn bfs_hits_owner_macro_uncached(&mut self, start_name: &str) -> Option<MacroReachEdge> {
        let mut queue: VecDeque<(&str, usize)> = VecDeque::new();
        let mut visited: HashSet<&str> = HashSet::new();

        queue.push_back((start_name, 1));
        visited.insert(start_name);

        while let Some((current_name, depth)) = queue.pop_front() {
            if depth > MAX_TRANSITIVE_DEPTH {
                continue;
            }
            let graph = self.graph;
            let invocations = graph
                .macro_invocations()
                .by_name
                .get(current_name)
                .map_or(&[][..], Vec::as_slice);
            for (current_fn, fn_invocations) in self.resolve(current_name).iter().zip(invocations) {
                for macro_invocation in fn_invocations {
                    if let Some(edge) = Self::macro_edge_for_invocation(
                        &mut self.macro_mention_memo,
                        macro_invocation,
                        &current_fn.file,
                        &current_fn.name,
                        &self.owner_name,
                        self.graph,
                    ) {
                        return Some(edge);
                    }
                }
                for call in calls_of(current_fn) {
                    if is_macro_call(call.name.as_str()) || call.name == self.owner_name {
                        continue;
                    }
                    if visited.insert(call.name.as_str()) {
                        queue.push_back((call.name.as_str(), depth + 1));
                    }
                }
            }
        }

        None
    }

    fn macro_edge_for_invocation(
        memo: &mut HashMap<String, bool>,
        invocation: &MacroInvocation,
        invocation_file: &std::path::Path,
        host: &str,
        owner_name: &str,
        graph: &ReachGraph<'_>,
    ) -> Option<MacroReachEdge> {
        if let Some(cached) = memo.get(invocation.name.as_str()) {
            if !*cached {
                return None;
            }
        } else {
            let mentions = graph.macro_definition_mentions_owner(&invocation.name, owner_name);
            memo.insert(invocation.name.clone(), mentions);
            if !mentions {
                return None;
            }
        }
        Some(MacroReachEdge {
            macro_name: invocation.name.clone(),
            macro_file: invocation_file.to_path_buf(),
            macro_line: invocation.line,
            macro_host: host.to_string(),
        })
    }
}

fn macro_invocations_in_text(text: &str, start_line: usize) -> Vec<MacroInvocation> {
    let mut invocations = Vec::new();
    for (offset, line) in text.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut cursor = 0usize;
        while cursor < bytes.len() {
            if bytes[cursor] == b'!'
                && next_non_ws_is_macro_delimiter(bytes, cursor.saturating_add(1))
                && let Some(name) = macro_name_before_bang(line, cursor)
            {
                invocations.push(MacroInvocation {
                    name,
                    line: start_line + offset,
                });
            }
            cursor += 1;
        }
    }
    invocations.sort_by(|left, right| left.line.cmp(&right.line).then(left.name.cmp(&right.name)));
    invocations.dedup_by(|left, right| left.line == right.line && left.name == right.name);
    invocations
}

fn macro_name_before_bang(line: &str, bang_index: usize) -> Option<String> {
    let bytes = line.as_bytes();
    let mut end = bang_index;
    while end > 0 && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }
    let mut start = end;
    while start > 0 && is_ascii_ident_byte(bytes[start - 1]) {
        start -= 1;
    }
    if start == end {
        return None;
    }
    if start > 0 && is_ascii_ident_byte(bytes[start - 1]) {
        return None;
    }
    line.get(start..end).map(ToString::to_string)
}

fn next_non_ws_is_macro_delimiter(bytes: &[u8], start: usize) -> bool {
    let mut cursor = start;
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    matches!(bytes.get(cursor), Some(b'(' | b'[' | b'{'))
}

#[cfg(test)]
fn source_macro_definition_mentions_owner(
    source: &str,
    macro_name: &str,
    owner_name: &str,
) -> bool {
    let scan = scan_macro_definitions(source, macro_name, owner_name);
    scan.same_name_count == 1 && scan.owner_mention_count == 1
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MacroDefinitionScan {
    same_name_count: usize,
    owner_mention_count: usize,
}

#[cfg(test)]
fn scan_macro_definitions(source: &str, macro_name: &str, owner_name: &str) -> MacroDefinitionScan {
    let definitions = macro_definitions_named(source, macro_name);
    MacroDefinitionScan {
        same_name_count: definitions.len(),
        owner_mention_count: definitions
            .iter()
            .flatten()
            .filter(|body| contains_identifier(body, owner_name))
            .count(),
    }
}

/// One entry per `macro_rules! macro_name` definition in `source`, holding
/// its body when it has one. A definition nested inside a counted body of
/// the same name is skipped; definitions inside other macros' bodies count.
/// The per-name scan the one-pass table in [`ReachGraph::build`] reproduces.
#[cfg(test)]
fn macro_definitions_named<'s>(source: &'s str, macro_name: &str) -> Vec<Option<&'s str>> {
    let marker = "macro_rules!";
    let mut definitions = Vec::new();
    let mut cursor = 0usize;

    while let Some(relative_start) = source.get(cursor..).and_then(|tail| tail.find(marker)) {
        let marker_start = cursor.saturating_add(relative_start);
        let name_start = skip_ascii_whitespace(source, marker_start.saturating_add(marker.len()));
        let name_end = ascii_ident_end(source, name_start);
        let Some(found_name) = source.get(name_start..name_end) else {
            break;
        };
        if found_name.is_empty() {
            cursor = marker_start.saturating_add(marker.len());
            continue;
        }

        if found_name == macro_name {
            if let Some((body_start, body_end)) = macro_body_range(source, name_end) {
                definitions.push(source.get(body_start..body_end));
                cursor = body_end;
                continue;
            }
            definitions.push(None);
        }

        cursor = name_end;
    }

    definitions
}

/// Add `source`'s `macro_rules!` bodies to `bodies`, and its definitions to
/// the per-name `definitions` table exactly as [`macro_definitions_named`]
/// would count them for each name.
fn add_macro_definitions<'s>(
    source: &'s str,
    bodies: &mut Vec<&'s str>,
    definitions: &mut HashMap<&'s str, Vec<Option<&'s str>>>,
) {
    // The per-name scan resumes after a counted body, so a same-name
    // definition nested in it is not counted again.
    let mut counted_until: HashMap<&str, usize> = HashMap::new();
    for definition in macro_definition_markers(source) {
        let body = definition.body.map(|(_, body)| body);
        bodies.extend(body);
        if counted_until
            .get(definition.name)
            .is_some_and(|&end| definition.marker_start < end)
        {
            continue;
        }
        if let Some((body_end, _)) = definition.body {
            counted_until.insert(definition.name, body_end);
        }
        definitions.entry(definition.name).or_default().push(body);
    }
}

struct MacroDefinitionMarker<'s> {
    marker_start: usize,
    name: &'s str,
    /// The body's end offset and text, when the name is followed by one.
    body: Option<(usize, &'s str)>,
}

/// Every named `macro_rules!` definition in `source`, in order, visiting each
/// marker (including ones nested in another body), so every definition
/// [`macro_definitions_named`] reads for any one name is among them.
fn macro_definition_markers(source: &str) -> Vec<MacroDefinitionMarker<'_>> {
    let marker = "macro_rules!";
    let mut definitions = Vec::new();
    let mut cursor = 0usize;
    while let Some(relative_start) = source.get(cursor..).and_then(|tail| tail.find(marker)) {
        let marker_start = cursor.saturating_add(relative_start);
        let name_start = skip_ascii_whitespace(source, marker_start.saturating_add(marker.len()));
        let name_end = ascii_ident_end(source, name_start);
        let Some(name) = source
            .get(name_start..name_end)
            .filter(|name| !name.is_empty())
        else {
            cursor = marker_start.saturating_add(marker.len());
            continue;
        };
        let body = macro_body_range(source, name_end).and_then(|(body_start, body_end)| {
            source
                .get(body_start..body_end)
                .map(|body| (body_end, body))
        });
        definitions.push(MacroDefinitionMarker {
            marker_start,
            name,
            body,
        });
        cursor = name_end;
    }
    definitions
}

fn skip_ascii_whitespace(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut cursor = start;
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    cursor
}

fn ascii_ident_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    let mut cursor = start;
    while cursor < bytes.len() && is_ascii_ident_byte(bytes[cursor]) {
        cursor += 1;
    }
    cursor
}

fn macro_body_range(source: &str, after_name: usize) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let body_start = skip_ascii_whitespace(source, after_name);
    let open = *bytes.get(body_start)?;
    let close = match open {
        b'{' => b'}',
        b'(' => b')',
        b'[' => b']',
        _ => return None,
    };
    let mut depth = 0usize;
    let mut cursor = body_start;
    while cursor < bytes.len() {
        match bytes[cursor] {
            byte if byte == open => {
                depth = depth.saturating_add(1);
            }
            byte if byte == close => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some((body_start, cursor.saturating_add(1)));
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    None
}

#[cfg(test)]
fn line_macro_rules_name(line: &str) -> Option<String> {
    let marker = "macro_rules!";
    let start = line.find(marker)?.saturating_add(marker.len());
    let suffix = line.get(start..)?.trim_start();
    let name_len = suffix
        .bytes()
        .take_while(|byte| is_ascii_ident_byte(*byte))
        .count();
    if name_len == 0 {
        return None;
    }
    suffix.get(..name_len).map(ToString::to_string)
}

fn contains_identifier(text: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    text.match_indices(needle).any(|(start, _)| {
        let end = start.saturating_add(needle.len());
        let before_ok = start == 0
            || !text
                .as_bytes()
                .get(start - 1)
                .is_some_and(|byte| is_ascii_ident_byte(*byte));
        let after_ok = end >= text.len()
            || !text
                .as_bytes()
                .get(end)
                .is_some_and(|byte| is_ascii_ident_byte(*byte));
        before_ok && after_ok
    })
}

fn is_ascii_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn calls_of(f: &FunctionSummary) -> &[CallFact] {
    &f.calls
}

/// Whether `call` is the function's own declaration line (`fn build(&self)`),
/// which call facts record under the function's own name. It is no evidence
/// that the function leads anywhere: without this, `Cache::build` would
/// "reach" through the name `build` (#5481). A real call to a same-named
/// function on another type (`self.queue.build()`) still counts.
fn is_own_declaration(call: &CallFact, function: &FunctionSummary) -> bool {
    call.name == function.name
        && contains_identifier(&call.text, "fn")
        && call.text.contains(&format!("fn {}", function.name))
}

/// Returns true when the callee name looks like a macro invocation - i.e. it
/// contains `!`. Lexical call extraction in ripr may or may not retain the
/// bang; we check containment to fail closed.
fn is_macro_call(name: &str) -> bool {
    name.contains('!')
}

/// The `static_limit_kind` a transitive witness names (RIPR-SPEC-0118): an
/// integration-test origin is a public-API path, anything else a helper
/// chain.
pub(in crate::analysis) fn transitive_reach_limit_kind(test_file: &Path) -> StaticLimitKind {
    if crate::analysis::rust_index::is_test_file(test_file) {
        StaticLimitKind::RustIntegrationPublicApiPathUnresolved
    } else {
        StaticLimitKind::RustTransitiveReachUnresolved
    }
}

/// The `static_limit_kind` a macro witness names: a macro in the test body
/// itself, or one on the path toward the owner.
pub(in crate::analysis) fn macro_reach_limit_kind(macro_host: &str) -> StaticLimitKind {
    if macro_host == MACRO_WITNESS_TEST_BODY_HOST {
        StaticLimitKind::RustMacroWrappedTestCallUnresolved
    } else {
        StaticLimitKind::RustMacroReachUnresolved
    }
}

/// The human/JSON message emitted as a stop-reason when the transitive
/// limitation fires. This is a named limitation, NOT a coverage claim.
pub(in crate::analysis) const RUST_TRANSITIVE_REACH_MESSAGE: &str = "ripr saw a test reaching public API that may call toward this change \
     through a transitive path it does not fully trace \
     (pub to pub(crate) helper chains, macros, or generics). \
     This is not a coverage assessment -- ripr cannot confirm or deny \
     that the change is observed.";

/// The human/JSON message emitted when a no_static_path finding hits a macro
/// boundary whose same-repo definition lexically mentions the changed owner.
/// This is a named limitation, NOT a coverage claim.
pub(in crate::analysis) const RUST_MACRO_REACH_MESSAGE: &str = "ripr saw a test reaching a Rust entry point whose path toward this change \
     stops at a macro invocation it does not expand. \
     This is not a coverage assessment -- ripr cannot confirm or deny \
     that the macro-generated path observes the change.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::FunctionSourceRole;
    use crate::analysis::facts::{CallFact, FileFacts, FunctionSummary, RustIndex, TestFact};
    use crate::domain::SymbolId;
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    fn make_fn(name: &str, calls: Vec<&str>) -> FunctionSummary {
        make_fn_with_body(name, calls, String::new())
    }

    fn make_fn_with_body(name: &str, calls: Vec<&str>, body: String) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("src/lib.rs::{name}")),
            name: name.to_string(),
            file: PathBuf::from("src/lib.rs"),
            start_line: 1,
            end_line: 10,
            body,
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
            source_role: FunctionSourceRole::Production,
            attrs: Vec::new(),
            impl_attrs: Vec::new(),
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
            item: Default::default(),
            impl_context: Default::default(),
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
            nested_fn_names: Vec::new(),
            let_bindings: Vec::new(),
        }
    }

    fn index_with(fns: Vec<FunctionSummary>, tests: Vec<TestFact>) -> RustIndex {
        index_with_source(fns, tests, String::new())
    }

    fn index_with_source(
        fns: Vec<FunctionSummary>,
        tests: Vec<TestFact>,
        source: String,
    ) -> RustIndex {
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
                used_lexical_fallback: false,
                module_declarations: Vec::new(),
                unresolved_property_macros: Vec::new(),
                role_provenance: Default::default(),
                source,
            },
        );
        RustIndex::from_owned(crate::analysis::facts::OwnedRustIndex {
            files,
            tests,
            functions: Vec::new(),
            workspace_authority: None,
            ..Default::default()
        })
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

    // (a'') Same-named functions: jiter's parser calls `decode` on a generic
    // decoder; two impls define `decode` and only one of them calls the
    // changed `decode_to_tape`. The walk must follow every `decode`, not the
    // one indexed first.
    #[test]
    fn given_same_named_functions_then_the_walk_follows_each_of_them() {
        let range_decode = make_fn("decode", vec!["decode_chunk"]);
        let string_decode = make_fn("decode", vec!["decode_to_tape"]);
        let parse = make_fn("parse_str", vec!["decode"]);
        let index = index_with(
            vec![range_decode, string_decode, parse],
            vec![make_test("test_partial_escape", vec!["parse_str"])],
        );

        let witness = find_transitive_witness("decode_to_tape", &index);
        assert_eq!(
            witness.as_ref().map(|w| w.entry_symbol.as_str()),
            Some("parse_str")
        );

        // Control: neither `decode` reaches the owner.
        let only_range = index_with(
            vec![
                make_fn("decode", vec!["decode_chunk"]),
                make_fn("decode", vec!["decode_bytes"]),
                make_fn("parse_str", vec!["decode"]),
            ],
            vec![make_test("test_partial_escape", vec!["parse_str"])],
        );
        assert!(find_transitive_witness("decode_to_tape", &only_range).is_none());
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

    fn make_method(self_type: &str, name: &str, calls: Vec<&str>) -> FunctionSummary {
        let mut function = make_fn(name, calls);
        function.id = SymbolId(format!("src/lib.rs::impl {self_type}::{name}"));
        function
    }

    /// Adds the declaration-line call fact the parser records under the
    /// function's own name (`fn build(&mut self) {`).
    fn with_declaration(mut function: FunctionSummary) -> FunctionSummary {
        function.calls.insert(
            0,
            CallFact {
                line: 1,
                name: function.name.clone(),
                text: format!("fn {}(&mut self) {{", function.name),
            },
        );
        function
    }

    fn with_call_text(mut function: FunctionSummary, name: &str, text: &str) -> FunctionSummary {
        for call in &mut function.calls {
            if call.name == name {
                call.text = text.to_string();
            }
        }
        function
    }

    fn with_body(mut test: TestFact, body: &str) -> TestFact {
        test.body = body.to_string();
        test
    }

    // (#5481) A unit test calling an unrelated type's same-named method must
    // not win on file order over the integration test whose body names the
    // type that reaches the owner. Only the ranking moves: both tests stay
    // candidates, so the count of others is unchanged.
    #[test]
    fn given_same_named_method_on_other_type_then_corroborated_witness_is_named() {
        let site_build = make_method("Site", "build", vec!["full_build"]);
        // Call facts include the function's own name; that must not count
        // as a path onward.
        let cache_build = with_declaration(make_method("Cache", "build", vec![]));
        let index = index_with(
            vec![site_build, cache_build],
            vec![
                with_body(
                    make_test_at("cache_builds", "src/render.rs", 24, vec!["new", "build"]),
                    "let mut c = Cache::new(); c.build(); assert!(c.is_built());",
                ),
                with_body(
                    make_test_at("atom_written", "tests/site.rs", 6, vec!["build"]),
                    "let site = Site { langs: Vec::new() }; assert!(site.build().is_empty());",
                ),
            ],
        );

        let witness = find_transitive_witness("full_build", &index);
        assert_eq!(
            witness.as_ref().map(|w| w.test_name.as_str()),
            Some("atom_written")
        );
        assert_eq!(
            witness.as_ref().map(|w| w.test_file.clone()),
            Some(PathBuf::from("tests/site.rs"))
        );
        assert_eq!(witness.as_ref().map(|w| w.other_test_count), Some(1));

        // Control: with no body naming `Site`, neither test is corroborated
        // and file order decides, as before.
        let uncorroborated = index_with(
            vec![
                make_method("Site", "build", vec!["full_build"]),
                make_method("Cache", "build", vec![]),
            ],
            vec![
                make_test_at("cache_builds", "src/render.rs", 24, vec!["build"]),
                make_test_at("atom_written", "tests/site.rs", 6, vec!["build"]),
            ],
        );
        assert_eq!(
            find_transitive_witness("full_build", &uncorroborated)
                .as_ref()
                .map(|w| w.test_name.as_str()),
            Some("cache_builds")
        );
    }

    // The per-test receiver memo is shared by every owner queried on one
    // index. `full_build` is reached through `Site::build` and `render_cache`
    // through `Cache::build`, so the same test answers differently for each
    // owner; a memo keyed on the method alone would carry the first owner's
    // answer into the second query and name `atom_written` for both.
    #[test]
    fn receiver_memo_answers_each_owner_as_a_fresh_index_does() {
        let index = index_with(
            vec![
                make_method("Site", "build", vec!["full_build"]),
                make_method("Cache", "build", vec!["render_cache"]),
            ],
            vec![
                with_body(
                    make_test_at("cache_builds", "src/render.rs", 24, vec!["new", "build"]),
                    "let mut c = Cache::new(); c.build(); assert!(c.is_built());",
                ),
                with_body(
                    make_test_at("atom_written", "tests/site.rs", 6, vec!["build"]),
                    "let site = Site { langs: Vec::new() }; assert!(site.build().is_empty());",
                ),
            ],
        );
        let witness_name = |reach: &TransitiveReachIndex<'_>, owner: &str| {
            reach.transitive_witness(owner).map(|w| w.test_name)
        };

        let shared = TransitiveReachIndex::new(&index);
        for (owner, expected) in [
            ("full_build", "atom_written"),
            ("render_cache", "cache_builds"),
        ] {
            assert_eq!(witness_name(&shared, owner).as_deref(), Some(expected));
            assert_eq!(
                witness_name(&shared, owner),
                witness_name(&TransitiveReachIndex::new(&index), owner)
            );
        }
    }

    // (#5481 review) A test that names the reaching type away from the call
    // is not corroborated: `cache.build()` does not resolve to `Site`. With
    // neither test corroborated, file order decides.
    #[test]
    fn given_type_named_away_from_the_call_then_the_test_is_not_corroborated() {
        let index = index_with(
            vec![
                make_method("Site", "build", vec!["full_build"]),
                with_declaration(make_method("Cache", "build", vec![])),
            ],
            vec![
                with_body(
                    make_test_at(
                        "site_from_fixture",
                        "src/a.rs",
                        3,
                        vec!["make_site", "build"],
                    ),
                    "let site = make_site(); assert!(site.build().is_empty());",
                ),
                with_body(
                    make_test_at(
                        "cache_with_unused_site",
                        "src/b.rs",
                        3,
                        vec!["new", "build"],
                    ),
                    "let _unused = Site { langs: Vec::new() }; let mut cache = Cache::new(); cache.build();",
                ),
            ],
        );

        let witness = find_transitive_witness("full_build", &index);
        assert_eq!(
            witness.as_ref().map(|w| w.test_name.as_str()),
            Some("site_from_fixture")
        );
        assert_eq!(witness.as_ref().map(|w| w.other_test_count), Some(1));
    }

    // (#5481 review) A type named only as a constructor argument is not the
    // receiver's type: `Cache::new(Site::default())` binds a `Cache`. The
    // cache test sorts first, so it would win if it counted as corroborated.
    #[test]
    fn given_type_only_in_constructor_argument_then_the_receiver_is_not_that_type() {
        let index = index_with(
            vec![
                make_method("Site", "build", vec!["full_build"]),
                with_declaration(make_method("Cache", "build", vec![])),
            ],
            vec![
                with_body(
                    make_test_at(
                        "cache_wraps_site",
                        "src/0_cache.rs",
                        3,
                        vec!["new", "build"],
                    ),
                    "let cache = Cache::new(Site::default()); cache.build();",
                ),
                with_body(
                    make_test_at("site_builds", "tests/site.rs", 3, vec!["new", "build"]),
                    "let site: Site = Site::new(); assert!(site.build().is_empty());",
                ),
            ],
        );

        let witness = find_transitive_witness("full_build", &index);
        assert_eq!(
            witness.as_ref().map(|w| w.test_name.as_str()),
            Some("site_builds")
        );
        assert_eq!(witness.as_ref().map(|w| w.other_test_count), Some(1));
    }

    // (#5481 review) `Site::build` calling `self.queue.build()` is a real call
    // onward, not the declaration of `build`, so a test on `Site` stays
    // corroborated when `Queue::build` is what reaches the owner.
    #[test]
    fn given_cross_type_same_named_call_then_the_caller_still_reaches() {
        let index = index_with(
            vec![
                with_call_text(
                    with_declaration(make_method("Site", "build", vec!["build"])),
                    "build",
                    "self.queue.build()",
                ),
                make_method("Queue", "build", vec!["full_build"]),
                with_declaration(make_method("Cache", "build", vec![])),
            ],
            vec![
                with_body(
                    make_test_at("cache_builds", "src/0_cache.rs", 3, vec!["new", "build"]),
                    "let _q = Queue::new(); let mut cache = Cache::new(); cache.build();",
                ),
                with_body(
                    make_test_at("site_builds", "src/site.rs", 3, vec!["new", "build"]),
                    "let site = Site::new(); assert!(site.build().is_empty());",
                ),
            ],
        );

        let witness = find_transitive_witness("full_build", &index);
        assert_eq!(
            witness.as_ref().map(|w| w.test_name.as_str()),
            Some("site_builds")
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

    #[test]
    fn transitive_reach_limitation_detail_names_edges_route_and_non_claim() {
        let witness = TransitiveWitness {
            test_name: "test_uses_outer".to_string(),
            test_file: PathBuf::from("tests/it.rs"),
            test_line: 12,
            entry_symbol: "outer".to_string(),
            other_test_count: 0,
        };

        let detail = transitive_reach_limitation_detail_lines(&witness, "inner");

        assert_eq!(
            detail[0],
            "limitation_last_established_edge: test `test_uses_outer` (tests/it.rs:12) -> entry `outer`"
        );
        assert_eq!(
            detail[1],
            "limitation_first_unresolved_edge: entry `outer` -> owner `inner` through a transitive Rust helper path"
        );
        assert_eq!(
            detail[2],
            "limitation_analyzer_route: analysis/rust-public-api-transitive-reach"
        );
        assert!(
            detail[3].starts_with("limitation_non_claim: named limitation only"),
            "{detail:?}"
        );
    }

    // (c-i) Path exists at exactly depth=5 -> witness captured (boundary is depth > 5 not >= 5).
    // test -> fn_a(1) -> fn_b(2) -> fn_c(3) -> fn_d(4) -> fn_e(5)
    // -> check fn_e.calls: includes inner.
    #[test]
    fn given_path_at_depth_5_then_witness_is_captured() {
        let fn_a = make_fn("fn_a", vec!["fn_b"]);
        let fn_b = make_fn("fn_b", vec!["fn_c"]);
        let fn_c = make_fn("fn_c", vec!["fn_d"]);
        let fn_d = make_fn("fn_d", vec!["fn_e"]);
        let fn_e = make_fn("fn_e", vec!["inner"]);
        let index = index_with(
            vec![fn_a, fn_b, fn_c, fn_d, fn_e],
            vec![make_test("test_depth5", vec!["fn_a"])],
        );
        let witness = find_transitive_witness("inner", &index);
        assert_eq!(
            witness.as_ref().map(|w| w.entry_symbol.as_str()),
            Some("fn_a")
        );
    }

    // (c-ii) Path at depth=6 -> exceeds MAX_TRANSITIVE_DEPTH=5, NOT found.
    // fn_e is popped at depth=5, its calls include fn_f -> push fn_f at depth=6.
    // fn_f is popped at depth=6, 6 > 5 -> continue. inner NOT reached.
    #[test]
    fn given_path_depth_6_then_witness_is_none() {
        let fn_a = make_fn("fn_a", vec!["fn_b"]);
        let fn_b = make_fn("fn_b", vec!["fn_c"]);
        let fn_c = make_fn("fn_c", vec!["fn_d"]);
        let fn_d = make_fn("fn_d", vec!["fn_e"]);
        let fn_e = make_fn("fn_e", vec!["fn_f"]);
        let fn_f = make_fn("fn_f", vec!["inner"]);
        let index = index_with(
            vec![fn_a, fn_b, fn_c, fn_d, fn_e, fn_f],
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
    fn given_empty_owner_then_witnesses_are_none() {
        let index = index_with_source(
            vec![make_fn("outer", vec!["inner"])],
            vec![make_test("test_uses_outer", vec!["outer"])],
            "macro_rules! call_inner { () => { inner() }; }".to_string(),
        );

        assert!(find_transitive_witness("", &index).is_none());
        assert!(find_macro_reach_witness("", &index).is_none());
    }

    #[test]
    fn given_entry_path_stops_at_owner_macro_then_macro_witness_is_captured() {
        let outer = make_fn_with_body(
            "outer",
            vec![],
            "pub fn outer(a: i32, b: i32) -> i32 {\n    call_inner!(a, b)\n}".to_string(),
        );
        let source = "macro_rules! call_inner {\n    ($a:expr, $b:expr) => { inner($a, $b) };\n}"
            .to_string();
        let index = index_with_source(
            vec![outer],
            vec![make_test("test_uses_outer", vec!["outer"])],
            source,
        );

        let witness = find_macro_reach_witness("inner", &index);
        assert_eq!(
            witness.as_ref().map(|w| w.test_name.as_str()),
            Some("test_uses_outer")
        );
        assert_eq!(
            witness.as_ref().map(|w| w.entry_symbol.as_str()),
            Some("outer")
        );
        assert_eq!(
            witness.as_ref().map(|w| w.macro_name.as_str()),
            Some("call_inner")
        );
        assert_eq!(
            witness.as_ref().map(|w| w.macro_file.clone()),
            Some(PathBuf::from("src/lib.rs"))
        );
        assert_eq!(witness.as_ref().map(|w| w.macro_line), Some(2));
        assert_eq!(
            witness.as_ref().map(|w| w.macro_host.as_str()),
            Some("outer")
        );
    }

    #[test]
    fn given_file_local_test_has_multiple_macro_candidates_then_first_is_stable() {
        let outer = make_fn_with_body(
            "outer",
            vec![],
            "pub fn outer() -> i32 {\n    call_inner!()\n}".to_string(),
        );
        let source = "macro_rules! call_inner {\n    () => { inner() };\n}\n\
             macro_rules! beta_inner {\n    () => { inner() };\n}"
            .to_string();
        let mut index = index_with_source(vec![outer], Vec::new(), source);
        let test = TestFact {
            body: "fn test_macro_entry() {\n    beta_inner!();\n}".to_string(),
            ..make_test_at("test_file_local", "tests/file_local.rs", 7, vec!["outer"])
        };
        let path = PathBuf::from("src/lib.rs");
        if let Some(mut file) = index.owned_file(&path) {
            file.tests.push(test);
            index.insert_file_only(path.clone(), file);
        }
        assert!(index.tests().is_empty());
        assert_eq!(index.files().at(&path).tests.len(), 1);

        let witness = find_macro_reach_witness("inner", &index);
        assert_eq!(
            witness.as_ref().map(|w| w.test_name.as_str()),
            Some("test_file_local")
        );
        assert_eq!(
            witness.as_ref().map(|w| w.entry_symbol.as_str()),
            Some("beta_inner!")
        );
        assert_eq!(witness.as_ref().map(|w| w.other_test_count), Some(0));
    }

    #[test]
    fn given_macro_walk_follows_helper_calls_before_macro() {
        let outer = make_fn("outer", vec!["vec!", "inner", "helper"]);
        let helper = make_fn_with_body(
            "helper",
            vec![],
            "fn helper() -> i32 {\n    call_inner!()\n}".to_string(),
        );
        let source =
            "macro_rules! call_inner {\n    () => { crate::internal::inner() };\n}".to_string();
        let index = index_with_source(
            vec![outer, helper],
            vec![make_test("test_uses_outer", vec!["outer"])],
            source,
        );

        let witness = find_macro_reach_witness("inner", &index);
        assert_eq!(
            witness.as_ref().map(|w| w.entry_symbol.as_str()),
            Some("outer")
        );
        assert_eq!(
            witness.as_ref().map(|w| w.macro_host.as_str()),
            Some("helper")
        );
    }

    #[test]
    fn given_macro_walk_exceeds_depth_or_missing_function_then_none() {
        let fn_a = make_fn("fn_a", vec!["fn_b", "missing_helper"]);
        let fn_b = make_fn("fn_b", vec!["fn_c"]);
        let fn_c = make_fn("fn_c", vec!["fn_d"]);
        let fn_d = make_fn("fn_d", vec!["fn_e"]);
        let fn_e = make_fn("fn_e", vec!["fn_f"]);
        let fn_f = make_fn_with_body(
            "fn_f",
            vec![],
            "fn fn_f() -> i32 {\n    call_inner!()\n}".to_string(),
        );
        let source = "macro_rules! call_inner {\n    () => { inner() };\n}".to_string();
        let index = index_with_source(
            vec![fn_a, fn_b, fn_c, fn_d, fn_e, fn_f],
            vec![make_test("test_too_deep", vec!["fn_a"])],
            source,
        );

        assert!(find_macro_reach_witness("inner", &index).is_none());
    }

    #[test]
    fn given_macro_definition_does_not_name_owner_then_macro_witness_is_none() {
        let outer = make_fn_with_body(
            "outer",
            vec![],
            "pub fn outer(a: i32, b: i32) -> i32 {\n    call_other!(a, b)\n}".to_string(),
        );
        let source = "macro_rules! call_other {\n    ($a:expr, $b:expr) => { other($a, $b) };\n}"
            .to_string();
        let index = index_with_source(
            vec![outer],
            vec![make_test("test_uses_outer", vec!["outer"])],
            source,
        );

        assert!(find_macro_reach_witness("inner", &index).is_none());
    }

    #[test]
    fn given_test_calls_macro_and_owner_then_macro_witness_is_none() {
        let index = index_with_source(
            Vec::new(),
            vec![make_test("test_direct_or_macro", vec!["vec!", "inner"])],
            "macro_rules! call_inner { () => { inner() }; }".to_string(),
        );

        assert!(find_macro_reach_witness("inner", &index).is_none());
    }

    #[test]
    fn given_test_invokes_owner_macro_directly_then_macro_witness_is_captured() {
        let source = "macro_rules! call_inner {\n    ($a:expr, $b:expr) => { inner($a, $b) };\n}"
            .to_string();
        let test = TestFact {
            body: "fn test_macro_entry() {\n    call_inner!(10, 3);\n}".to_string(),
            ..make_test("test_macro_entry", vec![])
        };
        let index = index_with_source(Vec::new(), vec![test], source);

        let witness = find_macro_reach_witness("inner", &index);
        assert_eq!(
            witness.as_ref().map(|w| w.entry_symbol.as_str()),
            Some("call_inner!")
        );
        assert_eq!(
            witness.as_ref().map(|w| w.macro_host.as_str()),
            Some(MACRO_WITNESS_TEST_BODY_HOST)
        );
    }

    #[test]
    fn macro_witness_pointer_uses_may_language_and_no_coverage_claim() {
        let witness = MacroReachWitness {
            test_name: "test_uses_outer".to_string(),
            test_file: PathBuf::from("tests/it.rs"),
            test_line: 4,
            entry_symbol: "outer".to_string(),
            macro_name: "call_inner".to_string(),
            macro_file: PathBuf::from("src/lib.rs"),
            macro_line: 6,
            macro_host: "outer".to_string(),
            other_test_count: 1,
        };
        let pointer = macro_reach_witness_pointer(&witness);

        assert!(pointer.contains("test_uses_outer"));
        assert!(pointer.contains("tests/it.rs:4"));
        assert!(pointer.contains("outer"));
        assert!(pointer.contains("call_inner!"));
        assert!(pointer.contains("src/lib.rs:6"));
        assert!(pointer.contains("may lead here"));
        assert!(pointer.contains("and 1 other test"));
        assert!(!pointer.contains("reaches"));
        assert!(!pointer.contains("covers"));
        assert!(!pointer.contains("exercise"));
    }

    #[test]
    fn macro_witness_pointer_reports_zero_and_plural_other_tests() {
        let mut witness = MacroReachWitness {
            test_name: "test_uses_outer".to_string(),
            test_file: PathBuf::from("tests\\it.rs"),
            test_line: 4,
            entry_symbol: "outer".to_string(),
            macro_name: "call_inner".to_string(),
            macro_file: PathBuf::from("src\\lib.rs"),
            macro_line: 6,
            macro_host: "outer".to_string(),
            other_test_count: 0,
        };
        let zero = macro_reach_witness_pointer(&witness);
        assert!(zero.contains("tests/it.rs:4"));
        assert!(zero.contains("src/lib.rs:6"));
        assert!(!zero.contains("other test"));

        witness.other_test_count = 2;
        assert!(macro_reach_witness_pointer(&witness).contains("and 2 other tests"));
    }

    #[test]
    fn macro_reach_limitation_detail_names_edges_route_and_non_claim() {
        let witness = MacroReachWitness {
            test_name: "test_uses_outer".to_string(),
            test_file: PathBuf::from("tests/it.rs"),
            test_line: 12,
            entry_symbol: "outer".to_string(),
            macro_name: "call_inner".to_string(),
            macro_file: PathBuf::from("src/lib.rs"),
            macro_line: 10,
            macro_host: "outer".to_string(),
            other_test_count: 0,
        };

        let detail = macro_reach_limitation_detail_lines(&witness, "inner");

        assert_eq!(
            detail[0],
            "limitation_last_established_edge: test `test_uses_outer` (tests/it.rs:12) -> entry `outer` -> macro `call_inner!` at src/lib.rs:10"
        );
        assert_eq!(
            detail[1],
            "limitation_first_unresolved_edge: macro `call_inner!` expansion toward owner `inner`"
        );
        assert_eq!(
            detail[2],
            "limitation_analyzer_route: analysis/rust-macro-aware-reach"
        );
        assert!(
            detail[3].starts_with("limitation_non_claim: named limitation only"),
            "{detail:?}"
        );
    }

    #[test]
    fn direct_test_macro_limitation_detail_names_direct_macro_edge() {
        let witness = MacroReachWitness {
            test_name: "test_macro_entry".to_string(),
            test_file: PathBuf::from("tests/it.rs"),
            test_line: 4,
            entry_symbol: "call_inner!".to_string(),
            macro_name: "call_inner".to_string(),
            macro_file: PathBuf::from("tests/it.rs"),
            macro_line: 5,
            macro_host: MACRO_WITNESS_TEST_BODY_HOST.to_string(),
            other_test_count: 0,
        };

        let pointer = macro_reach_witness_pointer(&witness);
        let detail = macro_reach_limitation_detail_lines(&witness, "inner");

        assert!(pointer.contains("directly invokes macro `call_inner!`"));
        assert_eq!(
            detail[0],
            "limitation_last_established_edge: test `test_macro_entry` (tests/it.rs:4) -> direct macro `call_inner!` at tests/it.rs:5"
        );
    }

    #[test]
    fn macro_invocation_parser_handles_delimiters_whitespace_and_invalid_bangs() {
        let invocations = macro_invocations_in_text(
            "call_inner ! (1);\narray_inner![a];\nblock_inner! { a }\nnot_macro! name\n!(missing)",
            10,
        );

        assert_eq!(
            invocations
                .iter()
                .map(|invocation| (invocation.name.as_str(), invocation.line))
                .collect::<Vec<_>>(),
            vec![("call_inner", 10), ("array_inner", 11), ("block_inner", 12),]
        );
    }

    #[test]
    fn macro_definition_scanner_requires_boundaries_and_target_macro() {
        assert_eq!(line_macro_rules_name("macro_rules! {"), None);
        assert!(!contains_identifier("innerish", "inner"));
        assert!(!contains_identifier("outer innerish", "inner"));
        assert!(contains_identifier("outer inner", "inner"));
        assert!(!contains_identifier("inner", ""));
        assert!(!source_macro_definition_mentions_owner(
            "fn before() { inner(); }\nmacro_rules! call_inner { () => { other() }; }\nfn after() { inner(); }",
            "call_inner",
            "inner",
        ));
    }

    #[test]
    fn macro_definition_scanner_fail_closes_on_duplicate_macro_names() {
        assert!(!source_macro_definition_mentions_owner(
            "macro_rules! call_inner { () => { other() }; }\n\
             macro_rules! call_inner { () => { inner() }; }",
            "call_inner",
            "inner",
        ));
    }

    #[test]
    fn macro_definition_scanner_handles_non_brace_body_delimiters() {
        assert!(source_macro_definition_mentions_owner(
            "macro_rules! call_inner ( () => { inner() }; );\nfn after() { other(); }",
            "call_inner",
            "inner",
        ));
        assert!(source_macro_definition_mentions_owner(
            "macro_rules! call_inner [ () => { inner() }; ];\nfn after() { other(); }",
            "call_inner",
            "inner",
        ));
        assert!(!source_macro_definition_mentions_owner(
            "macro_rules! call_inner ( () => { other() }; );\nfn after() { inner(); }",
            "call_inner",
            "inner",
        ));
        assert!(!source_macro_definition_mentions_owner(
            "macro_rules! call_inner [ () => { other() }; ];\nfn after() { inner(); }",
            "call_inner",
            "inner",
        ));
    }

    #[test]
    fn one_pass_macro_table_matches_the_per_name_scan() {
        let sources = [
            // A same-name definition nested in a counted body is skipped.
            "macro_rules! outer { () => { macro_rules! outer { () => { inner() } } } }",
            // Other macros' bodies are entered.
            "macro_rules! host { () => { macro_rules! guest { () => { inner() } } } }\n\
             macro_rules! guest { () => {} }",
            // A definition with no body is counted but stops nothing.
            "macro_rules! bare;\nmacro_rules! bare { () => { inner() } }",
            // A name that ends in the marker text, and an empty name.
            "macro_rules! xmacro_rules! { () => {} }\nmacro_rules! (oops)\n\
             macro_rules! after { [] => [ inner() ] }",
            // Unbalanced body, then a nested same name after a closed one.
            "macro_rules! a { () => { macro_rules! a ( ) } }\nmacro_rules! a { ",
            // A nested same name with no body, and a nested one whose `(`
            // range runs past the outer body's end.
            "macro_rules! b { macro_rules! b; }\nmacro_rules! b { inner() }",
            "macro_rules! c { macro_rules! c ( } ) }\nmacro_rules! c { inner() }",
            "",
        ];
        for source in sources {
            let mut bodies = Vec::new();
            let mut table: HashMap<&str, Vec<Option<&str>>> = HashMap::new();
            add_macro_definitions(source, &mut bodies, &mut table);
            let names: BTreeSet<&str> = macro_definition_markers(source)
                .iter()
                .map(|definition| definition.name)
                .chain(["inner", "missing"])
                .collect();
            for name in names {
                assert_eq!(
                    table.get(name).cloned().unwrap_or_default(),
                    macro_definitions_named(source, name),
                    "{name} in {source:?}"
                );
            }
        }
        // The nested same-name case really is one entry, so a table that
        // counts every marker fails above.
        let mut table = HashMap::new();
        add_macro_definitions(sources[0], &mut Vec::new(), &mut table);
        assert_eq!(table.get("outer").map(Vec::len), Some(1));
    }

    /// The removed per-callee forward walk, kept as the reference the reverse
    /// sweep must match.
    fn forward_reaches(index: &RustIndex, start: &str, owner: &str) -> bool {
        let mut by_name: HashMap<&str, Vec<&FunctionSummary>> = HashMap::new();
        for function in index
            .files()
            .values()
            .flat_map(|file| file.functions.iter())
        {
            if !function.source_role.is_evidence_role() {
                by_name
                    .entry(function.name.as_str())
                    .or_default()
                    .push(function);
            }
        }
        let mut queue: VecDeque<(&str, usize)> = VecDeque::from([(start, 1)]);
        let mut visited: HashSet<&str> = HashSet::from([start]);
        while let Some((name, depth)) = queue.pop_front() {
            if depth > MAX_TRANSITIVE_DEPTH {
                continue;
            }
            for function in by_name.get(name).into_iter().flatten() {
                for call in &function.calls {
                    if is_macro_call(&call.name) {
                        continue;
                    }
                    if call.name == owner {
                        return true;
                    }
                    if visited.insert(call.name.as_str()) {
                        queue.push_back((call.name.as_str(), depth + 1));
                    }
                }
            }
        }
        false
    }

    #[test]
    fn reverse_reach_set_matches_the_forward_walk_from_every_name() {
        // A five-hop chain (in range), a six-hop chain (out of range), a
        // cycle, a macro edge, a duplicate name, and an edge into the owner
        // from beyond the owner itself.
        let fns = vec![
            make_fn("a1", vec!["a2"]),
            make_fn("a2", vec!["a3"]),
            make_fn("a3", vec!["a4"]),
            make_fn("a4", vec!["a5"]),
            make_fn("a5", vec!["owner"]),
            make_fn("b1", vec!["b2"]),
            make_fn("b2", vec!["a1"]),
            make_fn("c1", vec!["c2"]),
            make_fn("c2", vec!["c1", "a4"]),
            make_fn("m1", vec!["owner!", "vec!"]),
            make_fn("dup", vec!["nothing"]),
            make_fn("dup", vec!["a5"]),
            make_fn("owner", vec!["after"]),
            make_fn("after", vec!["owner"]),
        ];
        let index = index_with(fns, vec![make_test("t", vec!["b1"])]);
        let graph = ReachGraph::build(&index);
        let reaching = graph.names_reaching("owner");
        let names = [
            "a1", "a2", "a3", "a4", "a5", "b1", "b2", "c1", "c2", "m1", "dup", "after", "nothing",
        ];
        for name in names {
            assert_eq!(
                reaching.contains(name),
                forward_reaches(&index, name, "owner"),
                "{name}"
            );
        }
        // Discriminating: a1 is five hops out, b2 six, b1 seven.
        assert!(reaching.contains("a1"));
        assert!(!reaching.contains("b2"));
        assert!(!reaching.contains("m1"));
        assert!(reaching.contains("dup"));
    }

    #[test]
    fn transitive_reach_limit_kind_names_integration_test_path() {
        assert_eq!(
            transitive_reach_limit_kind(Path::new("tests/version_req.rs")),
            StaticLimitKind::RustIntegrationPublicApiPathUnresolved
        );
        assert_eq!(
            transitive_reach_limit_kind(Path::new("src/lib.rs")),
            StaticLimitKind::RustTransitiveReachUnresolved
        );
    }

    #[test]
    fn macro_reach_limit_kind_names_direct_test_body_macro_path() {
        assert_eq!(
            macro_reach_limit_kind(MACRO_WITNESS_TEST_BODY_HOST),
            StaticLimitKind::RustMacroWrappedTestCallUnresolved
        );
        assert_eq!(
            macro_reach_limit_kind("outer"),
            StaticLimitKind::RustMacroReachUnresolved
        );
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
