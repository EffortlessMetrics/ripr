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

/// Finds a deterministic macro-blocked witness for a `no_static_path` Rust
/// finding after the direct and bounded transitive checks found no confirmed
/// lexical path.
///
/// The witness only fires when a same-repo `macro_rules!` definition lexically
/// mentions the changed owner. This names the unresolved macro edge without
/// expanding it and without changing classification.
pub(in crate::analysis) fn find_macro_reach_witness(
    owner_name: &str,
    index: &RustIndex,
) -> Option<MacroReachWitness> {
    if owner_name.is_empty() {
        return None;
    }

    let all_tests = collect_all_tests(index);
    let prod_fns: Vec<&FunctionSummary> = index
        .files
        .values()
        .flat_map(|file| file.functions.iter().filter(|f| !f.is_test))
        .collect();

    let mut witnesses: Vec<MacroWitnessCandidate> = Vec::new();
    for test in &all_tests {
        let mut found: Vec<(String, MacroReachEdge)> = Vec::new();

        for macro_invocation in macro_invocations_in_text(&test.body, test.start_line) {
            if let Some(edge) = macro_edge_for_invocation(
                &macro_invocation,
                &test.file,
                "test body",
                owner_name,
                index,
            ) {
                found.push((format!("{}!", macro_invocation.name), edge));
            }
        }

        for callee in &test.calls {
            if is_macro_call(&callee.name) || callee.name == owner_name {
                continue;
            }
            if let Some(edge) = bfs_hits_owner_macro(&callee.name, owner_name, &prod_fns, index) {
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

fn bfs_hits_owner_macro(
    start_name: &str,
    owner_name: &str,
    prod_fns: &[&FunctionSummary],
    index: &RustIndex,
) -> Option<MacroReachEdge> {
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    let mut visited: HashSet<String> = HashSet::new();

    queue.push_back((start_name.to_string(), 1));
    visited.insert(start_name.to_string());

    while let Some((current_name, depth)) = queue.pop_front() {
        if depth > MAX_TRANSITIVE_DEPTH {
            continue;
        }
        let Some(current_fn) = find_prod_fn_by_name(&current_name, prod_fns) else {
            continue;
        };
        for macro_invocation in macro_invocations_in_text(&current_fn.body, current_fn.start_line) {
            if let Some(edge) = macro_edge_for_invocation(
                &macro_invocation,
                &current_fn.file,
                &current_fn.name,
                owner_name,
                index,
            ) {
                return Some(edge);
            }
        }
        for call in calls_of(current_fn) {
            if is_macro_call(call.name.as_str()) || call.name == owner_name {
                continue;
            }
            if visited.insert(call.name.clone()) {
                queue.push_back((call.name.clone(), depth + 1));
            }
        }
    }

    None
}

fn macro_edge_for_invocation(
    invocation: &MacroInvocation,
    invocation_file: &std::path::Path,
    host: &str,
    owner_name: &str,
    index: &RustIndex,
) -> Option<MacroReachEdge> {
    macro_definition_mentions_owner(index, &invocation.name, owner_name).then(|| MacroReachEdge {
        macro_name: invocation.name.clone(),
        macro_file: invocation_file.to_path_buf(),
        macro_line: invocation.line,
        macro_host: host.to_string(),
    })
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

fn macro_definition_mentions_owner(index: &RustIndex, macro_name: &str, owner_name: &str) -> bool {
    let mut same_name_count = 0usize;
    let mut owner_mention_count = 0usize;
    for file in index.files.values() {
        let scan = scan_macro_definitions(&file.source, macro_name, owner_name);
        same_name_count = same_name_count.saturating_add(scan.same_name_count);
        owner_mention_count = owner_mention_count.saturating_add(scan.owner_mention_count);
    }

    same_name_count == 1 && owner_mention_count == 1
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MacroDefinitionScan {
    same_name_count: usize,
    owner_mention_count: usize,
}

fn scan_macro_definitions(source: &str, macro_name: &str, owner_name: &str) -> MacroDefinitionScan {
    let marker = "macro_rules!";
    let mut scan = MacroDefinitionScan::default();
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
            scan.same_name_count = scan.same_name_count.saturating_add(1);
            if let Some((body_start, body_end)) = macro_body_range(source, name_end) {
                if source
                    .get(body_start..body_end)
                    .is_some_and(|body| contains_identifier(body, owner_name))
                {
                    scan.owner_mention_count = scan.owner_mention_count.saturating_add(1);
                }
                cursor = body_end;
                continue;
            }
        }

        cursor = name_end;
    }

    scan
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
    use crate::analysis::facts::{CallFact, FileFacts, FunctionSummary, RustIndex, TestFact};
    use crate::domain::SymbolId;
    use std::collections::BTreeMap;
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
                source,
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
        if let Some(file) = index.files.values_mut().next() {
            file.tests.push(test);
        }

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
        let fn_d = make_fn_with_body(
            "fn_d",
            vec![],
            "fn fn_d() -> i32 {\n    call_inner!()\n}".to_string(),
        );
        let source = "macro_rules! call_inner {\n    () => { inner() };\n}".to_string();
        let index = index_with_source(
            vec![fn_a, fn_b, fn_c, fn_d],
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
            Some("test body")
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
