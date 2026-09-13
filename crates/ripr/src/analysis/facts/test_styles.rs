use super::cfg_predicates;
use super::{FileFacts, FunctionFact, FunctionSourceRole, RustIndex, TestFact};
use crate::analysis::extract::extract_assertions;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct FunctionKey {
    file: PathBuf,
    start_line: usize,
    end_line: usize,
    name: String,
    body: String,
}

impl FunctionKey {
    fn from_function(function: &FunctionFact) -> Self {
        Self {
            file: function.file.clone(),
            start_line: function.start_line,
            end_line: function.end_line,
            name: function.name.clone(),
            body: function.body.clone(),
        }
    }

    fn from_test(test: &TestFact) -> Self {
        Self {
            file: test.file.clone(),
            start_line: test.start_line,
            end_line: test.end_line,
            name: test.name.clone(),
            body: test.body.clone(),
        }
    }
}

/// Reconcile parser-backed and lexical-fallback test facts through one exact
/// attribute vocabulary. This runs immediately after index construction so
/// every index consumer sees the same executable-test role regardless of
/// which syntax producer handled the file.
pub(super) fn normalize_index_test_styles(index: &mut RustIndex) {
    for facts in index.files.values_mut() {
        normalize_file_test_styles(facts);
    }

    let mut role_by_function = BTreeMap::new();
    let mut test_by_function = BTreeMap::new();
    for facts in index.files.values() {
        for function in &facts.functions {
            role_by_function.insert(FunctionKey::from_function(function), function.source_role);
        }
        for test in &facts.tests {
            test_by_function.insert(FunctionKey::from_test(test), test.clone());
        }
    }

    for function in &mut index.functions {
        if let Some(role) = role_by_function.get(&FunctionKey::from_function(function)) {
            function.source_role = *role;
        }
    }

    let mut normalized_tests = Vec::new();
    for function in &index.functions {
        if !function.source_role.is_evidence_role() {
            continue;
        }
        if let Some(test) = test_by_function.remove(&FunctionKey::from_function(function)) {
            normalized_tests.push(test);
        }
    }
    index.tests = normalized_tests;
}

fn normalize_file_test_styles(facts: &mut FileFacts) {
    let mut existing_tests = std::mem::take(&mut facts.tests)
        .into_iter()
        .map(|test| ((test.start_line, test.name.clone()), test))
        .collect::<BTreeMap<_, _>>();
    let lexical_lines = facts
        .used_lexical_fallback
        .then(|| facts.source.lines().collect::<Vec<_>>());
    let mut normalized_tests = Vec::new();

    for function in &mut facts.functions {
        let key = (function.start_line, function.name.clone());
        let existing_test = existing_tests.remove(&key);
        let has_test_attribute = match lexical_lines.as_deref() {
            Some(lines) => {
                attributes_define_test(lexical_attributes_before(lines, function.start_line))
            }
            None => attributes_define_test(function.attrs.iter().map(String::as_str)),
        };

        // Parser-backed cfg(test) helpers are evidence-role functions without
        // executable TestFacts. Preserve that producer-owned distinction while
        // correcting prefix lookalikes that arrived with a TestFact.
        let preserve_cfg_test_role = !has_test_attribute
            && function.source_role.is_evidence_role()
            && is_inside_cfg_test_module(&facts.source, function.start_line);
        // The normalizer's role law (#3531): the exact attribute vocabulary
        // makes the function an executable test, the preserved cfg-test walk
        // keeps evidence-only helper role, and everything else demotes to
        // production — the same bit the boolean recomputed before the typed
        // role existed. A promotion-claimed expansion keeps its explicit
        // test-case provenance; the normalizer credits the same exact
        // attribute family, so this only preserves which producer said so.
        let promotion_claimed_expansion =
            function.source_role == FunctionSourceRole::ParameterizedExpansion;
        function.source_role = if has_test_attribute {
            if promotion_claimed_expansion {
                FunctionSourceRole::ParameterizedExpansion
            } else {
                FunctionSourceRole::TestAttribute
            }
        } else if preserve_cfg_test_role {
            FunctionSourceRole::CfgTestModule
        } else {
            FunctionSourceRole::Production
        };

        if has_test_attribute {
            normalized_tests.push(match existing_test {
                Some(test) => test,
                None => test_fact_from_function(function),
            });
        }
    }

    facts.tests = normalized_tests;
}

fn test_fact_from_function(function: &FunctionFact) -> TestFact {
    TestFact {
        name: function.name.clone(),
        file: function.file.clone(),
        start_line: function.start_line,
        end_line: function.end_line,
        body: function.body.clone(),
        calls: function.calls.clone(),
        assertions: extract_assertions(&function.body, function.start_line),
        literals: function.literals.clone(),
        attrs: function.attrs.clone(),
        // #3727 Slice A: the reconciled test mirrors the function's shadow
        // facts — same body, same decisions.
        nested_fn_names: function.nested_fn_names.clone(),
        let_bindings: function.let_bindings.clone(),
    }
}

fn attributes_define_test<'attribute>(
    attributes: impl IntoIterator<Item = &'attribute str>,
) -> bool {
    attributes.into_iter().any(|attribute| {
        normalized_test_attribute_path(attribute)
            .as_deref()
            .is_some_and(is_test_attribute_path)
    })
}

fn lexical_attributes_before<'source>(
    lines: &[&'source str],
    start_line: usize,
) -> Vec<&'source str> {
    let function_index = start_line.saturating_sub(1).min(lines.len());
    let mut attributes = Vec::new();

    for line in lines[..function_index].iter().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("#[") {
            attributes.push(trimmed);
            continue;
        }
        break;
    }

    attributes.reverse();
    attributes
}

/// Line walk that preserves producer-owned cfg-test evidence roles. The
/// cfg-term semantics come from the one shared authority
/// (`cfg_predicates`, #3530) — the same source the parser producer
/// consumes — so this walk can never disagree with the producer about
/// whether an attribute gates its module on test. Attributes spanning
/// multiple lines are joined before classification; only exact bounded
/// forms the walk can prove grant credit, and everything else fails closed.
fn is_inside_cfg_test_module(source: &str, function_start_line: usize) -> bool {
    let lines: Vec<&str> = source
        .lines()
        .take(function_start_line.saturating_sub(1))
        .collect();
    let mut scopes = Vec::new();
    let mut pending_cfg_test = false;
    let mut index = 0usize;

    while index < lines.len() {
        let remainder_storage;
        let line: &str = if lines[index].trim_start().starts_with("#[") {
            match join_leading_attribute(&lines[index..]) {
                Some((attribute, mut remainder, consumed_lines)) => {
                    if cfg_predicates::attribute_requires_test(&attribute) {
                        pending_cfg_test = true;
                    }
                    // The closing line's remainder is processed below; the
                    // loop's own `index += 1` then moves past it.
                    index += consumed_lines.saturating_sub(1);
                    // One line can carry several attributes; the shared
                    // splitter classifies each leading attribute in turn.
                    while remainder.trim_start().starts_with("#[") {
                        let Some((next_attribute, rest)) =
                            cfg_predicates::split_leading_attribute(&remainder)
                        else {
                            break;
                        };
                        if cfg_predicates::attribute_requires_test(next_attribute) {
                            pending_cfg_test = true;
                        }
                        remainder = rest.to_string();
                    }
                    remainder_storage = remainder;
                    remainder_storage.as_str()
                }
                None => lines[index],
            }
        } else {
            lines[index]
        };

        let trimmed = line.trim();
        let declares_cfg_test_module =
            pending_cfg_test && trimmed.contains("mod ") && trimmed.contains('{');
        let mut module_opened = false;
        for character in line.chars() {
            match character {
                '{' => {
                    scopes.push(declares_cfg_test_module && !module_opened);
                    module_opened = true;
                }
                '}' => {
                    scopes.pop();
                }
                _ => {}
            }
        }
        if declares_cfg_test_module || (!trimmed.starts_with("#[") && !trimmed.is_empty()) {
            pending_cfg_test = false;
        }
        index += 1;
    }

    scopes.iter().any(|is_cfg_test_module| *is_cfg_test_module)
}

/// Joins continuation lines until the leading attribute's closing bracket so
/// multi-line attribute spellings the parser accepts stay visible to this
/// walk. Returns the attribute text, the unconsumed remainder of the closing
/// line, and how many lines the attribute spans. `None` (no complete
/// attribute) fails closed with no gate credit.
fn join_leading_attribute(lines: &[&str]) -> Option<(String, String, usize)> {
    // An unclosed `#[` must not swallow the rest of the file: real
    // attributes span a handful of lines, so past this bound the walk fails
    // closed for the candidate instead of joining unrelated lines.
    const MAX_JOINED_LINES: usize = 32;
    let mut joined = String::new();
    for (offset, line) in lines.iter().enumerate() {
        if offset >= MAX_JOINED_LINES {
            return None;
        }
        joined.push_str(line);
        joined.push('\n');
        if let Some((attribute, remainder)) = cfg_predicates::split_leading_attribute(&joined) {
            return Some((attribute.to_string(), remainder.to_string(), offset + 1));
        }
    }
    None
}

pub(super) fn normalized_test_attribute_path(attribute: &str) -> Option<String> {
    let body = attribute.trim().strip_prefix("#[")?;
    let closing = body.rfind(']')?;
    let trailing = body.get(closing + 1..)?.trim();
    if !trailing.is_empty() && !trailing.starts_with("//") && !trailing.starts_with("/*") {
        return None;
    }
    let head = body.get(..closing)?.trim();
    if !attribute_arguments_are_balanced(head) {
        return None;
    }
    let path = head
        .split('(')
        .next()?
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    let path = path.trim_start_matches("::");
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

fn attribute_arguments_are_balanced(head: &str) -> bool {
    let Some(opening) = head.find('(') else {
        return true;
    };
    let mut depth = 0usize;
    for character in head[opening..].chars() {
        match character {
            '(' => depth = depth.saturating_add(1),
            ')' => {
                let Some(next_depth) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next_depth;
            }
            _ => {}
        }
    }
    depth == 0
}

fn is_test_attribute_path(path: &str) -> bool {
    matches!(
        path,
        "test"
            | "tokio::test"
            | "async_std::test"
            | "rstest"
            | "rstest::rstest"
            | "quickcheck"
            | "quickcheck_macros::quickcheck"
            | "wasm_bindgen_test"
            | "wasm_bindgen_test::wasm_bindgen_test"
            | "test_case"
            | "test_case::test_case"
            | "ntest::test_case"
            | "test_matrix"
            | "test_case::test_matrix"
    )
}

#[cfg(test)]
mod tests;
