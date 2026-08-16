//! RIPR-SPEC-0133: detect assertion-shaped owners (oracles).
//!
//! The R-I-P-R guidance presumes the changed owner is code *under* test. When
//! the changed owner is itself an assertion helper, advice like "add a
//! co-located test that observes the changed owner" asks for a test of the
//! oracle. This module detects that shape conservatively so
//! `decision::recommended_next_step` can reframe the guidance; it never
//! changes the exposure class.
//!
//! The rule is explainable in one sentence (see
//! `crate::domain::ASSERTION_SHAPED_OWNER_REASON`): the owner's body contains
//! at least one assert-family macro (`assert!`, `assert_eq!`, `assert_ne!`,
//! `debug_assert!`, `debug_assert_eq!`, `debug_assert_ne!`) or `.expect(`
//! call, every non-declaration, non-control statement in the body is one of
//! those, and no non-test function in the index calls it.
//!
//! Fail-closed on purpose: any unrecognized statement, and any non-test caller
//! (even a bare-name token coincidence), disqualifies the owner. Under-emit is
//! preferred over over-emit.
//!
//! Caller test-ness reuses the shared production-function predicate
//! (`!is_test && !is_test_file(file)`, as in
//! `test_grip_evidence::related_tests::context`): a caller located under
//! `tests/` counts as a test caller even when it is a plain helper fn without
//! `#[test]`. A plain helper inside a `#[cfg(test)]` module in a `src/` file
//! is NOT visible to this predicate (the index marks test-ness by attribute
//! only), so such a caller still blocks the reframe — the fail-closed
//! direction.

use super::super::rust_index::{FunctionSummary, RustIndex, is_test_file};

/// Assert-family macros in scope for RIPR-SPEC-0133. `assert_matches!` and
/// snapshot macros are deliberately excluded: a body using them leaves an
/// unrecognized statement, which keeps the detection fail-closed.
const ASSERT_FAMILY_MACROS: [&str; 6] = [
    "assert!(",
    "assert_eq!(",
    "assert_ne!(",
    "debug_assert!(",
    "debug_assert_eq!(",
    "debug_assert_ne!(",
];

pub(in crate::analysis) fn is_assertion_shaped_owner(
    owner: &FunctionSummary,
    index: &RustIndex,
) -> bool {
    body_is_assertion_dominated(&owner.body) && !has_non_test_caller(owner, index)
}

/// A non-test caller is any function in the index (other than the owner
/// itself) that the shared production-function predicate counts as production
/// (`!is_test && !is_test_file(file)`) and whose call facts mention the
/// owner's bare name. Bare-name matching can collide with a different
/// same-named function, but a collision only *blocks* the oracle reframe —
/// the fail-closed direction.
fn has_non_test_caller(owner: &FunctionSummary, index: &RustIndex) -> bool {
    index.functions.iter().any(|function| {
        !function.is_test
            && !is_test_file(&function.file)
            && function.id != owner.id
            && function.calls.iter().any(|call| call.name == owner.name)
    })
}

/// Dominance rule: at least one assert-family statement, and zero statements
/// that are neither declarations (`let`), control flow, structure, nor
/// assert-family calls.
///
/// Statements are accumulated line-by-line until a line ends with `;`, `{`, or
/// `}` (or closes a block). There is no paren-depth tracking: a multi-line
/// statement simply keeps accumulating, and the joined text is classified as a
/// whole, so char literals like `'('` cannot skew the scan. A statement the
/// scanner cannot recognize counts as `Other` and blocks the detection —
/// fail-closed. A single-line compound statement (a block that opens and
/// closes on the same line, e.g. `if cond { side_effect(); }`) is classified
/// by its inner fragments so a non-assert call cannot hide inside a control
/// header.
fn body_is_assertion_dominated(body: &str) -> bool {
    let mut assert_statements = 0usize;
    let mut other_statements = 0usize;
    let mut statement = String::new();

    for raw_line in body.lines() {
        let line = strip_comments_and_strings(raw_line);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !statement.is_empty() {
            statement.push(' ');
        }
        statement.push_str(trimmed);

        let completes = trimmed.ends_with(';')
            || trimmed.ends_with('{')
            || trimmed.ends_with('}')
            || trimmed.starts_with('}');
        if completes {
            record_statement(&statement, &mut assert_statements, &mut other_statements);
            statement.clear();
        }
    }
    if !statement.is_empty() {
        record_statement(&statement, &mut assert_statements, &mut other_statements);
    }

    assert_statements >= 1 && other_statements == 0
}

fn record_statement(statement: &str, assert_statements: &mut usize, other_statements: &mut usize) {
    match classify_statement(statement) {
        StatementKind::Assert => *assert_statements += 1,
        StatementKind::Other => *other_statements += 1,
        StatementKind::Ignorable => {}
    }
}

enum StatementKind {
    Assert,
    Other,
    Ignorable,
}

fn classify_statement(statement: &str) -> StatementKind {
    if let Some(kind) = classify_compound_statement(statement) {
        return kind;
    }
    if contains_assert_family_call(statement) {
        return StatementKind::Assert;
    }
    let mut rest = statement;
    loop {
        let stripped = rest
            .strip_prefix("pub(crate) ")
            .or_else(|| rest.strip_prefix("pub "))
            .or_else(|| rest.strip_prefix("async "))
            .or_else(|| rest.strip_prefix("unsafe "))
            .or_else(|| rest.strip_prefix("const "));
        match stripped {
            Some(next) => rest = next,
            None => break,
        }
    }
    // The enclosing `fn` signature, declarations, and control-flow headers do
    // not count for or against dominance.
    let ignorable = rest.starts_with("fn ")
        || rest.starts_with("let ")
        || rest.starts_with('}')
        || rest.starts_with('#')
        || CONTROL_PREFIXES
            .iter()
            .any(|prefix| rest.starts_with(prefix));
    if ignorable {
        StatementKind::Ignorable
    } else {
        StatementKind::Other
    }
}

// Every entry carries its word boundary (trailing space or the structural
// `}`): a boundary-less "else" would also match `elsewhere(...)` and
// "loop" would match `loopback(...)`, wrongly ignoring real calls
// (#2170 review).
const CONTROL_PREFIXES: [&str; 7] = ["for ", "if ", "else ", "match ", "while ", "loop ", "}"];

/// Classify a single-line compound statement (block opens and closes within
/// the accumulated statement, e.g. `if cond { side_effect(); }`) by its inner
/// fragments. Returns `None` for statements that are not single-line
/// compounds. Inner fragments must all be empty, declarations, control
/// keywords, or assert-family calls; anything else is `Other` — a side effect
/// must not hide inside a control header. Fail-closed: match arms like
/// `_ => {}` are not control-headed and fall through to the caller, which
/// counts them as `Other`.
fn classify_compound_statement(statement: &str) -> Option<StatementKind> {
    let control_headed = CONTROL_PREFIXES
        .iter()
        .any(|prefix| statement.starts_with(prefix));
    if !control_headed || !statement.contains('{') || !statement.ends_with('}') {
        return None;
    }
    let open = statement.find('{')? + 1;
    let close = statement.rfind('}')?;
    if open >= close {
        return Some(StatementKind::Ignorable);
    }
    let inner = &statement[open..close];
    let mut saw_assert = false;
    for fragment in inner.split([';', '{', '}']) {
        let fragment = fragment.trim();
        if fragment.is_empty() || fragment == "else" || fragment.starts_with("let ") {
            continue;
        }
        if contains_assert_family_call(fragment) {
            saw_assert = true;
            continue;
        }
        return Some(StatementKind::Other);
    }
    Some(if saw_assert {
        StatementKind::Assert
    } else {
        StatementKind::Ignorable
    })
}

fn contains_assert_family_call(text: &str) -> bool {
    ASSERT_FAMILY_MACROS
        .iter()
        .any(|needle| contains_word_call(text, needle))
        || text.contains(".expect(")
}

/// Word-boundary match so `my_assert!(` does not count as `assert!(`. The
/// `debug_assert*` needles match at their own start; the `_` before `assert!(`
/// inside them correctly fails the boundary for the shorter needle.
fn contains_word_call(text: &str, needle: &str) -> bool {
    let mut start = 0usize;
    while let Some(pos) = text[start..].find(needle) {
        let index = start + pos;
        let boundary = index == 0
            || !text[..index]
                .chars()
                .last()
                .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        if boundary {
            return true;
        }
        start = index + 1;
    }
    false
}

/// Blank out `//` comments and string-literal contents so an assertion
/// mentioned in prose (a comment, or a failure message naming another macro)
/// does not count as a statement.
fn strip_comments_and_strings(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            out.push(' ');
            continue;
        }
        if ch == '/' && chars.peek().is_some_and(|next| *next == '/') {
            break;
        }
        if ch == '"' {
            in_string = true;
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::rust_index::CallFact;
    use crate::domain::SymbolId;
    use std::path::PathBuf;

    fn function(
        file: &str,
        name: &str,
        body: &str,
        is_test: bool,
        calls: Vec<&str>,
    ) -> FunctionSummary {
        FunctionSummary {
            id: SymbolId(format!("{file}::{name}")),
            name: name.to_string(),
            file: PathBuf::from(file),
            start_line: 1,
            end_line: 1,
            body: body.to_string(),
            calls: calls
                .into_iter()
                .map(|call| CallFact {
                    line: 1,
                    name: call.to_string(),
                    text: format!("{call}()"),
                })
                .collect(),
            returns: Vec::new(),
            literals: Vec::new(),
            is_test,
            attrs: Vec::new(),
        }
    }

    const HAWK_SHAPED_BODY: &str = "fn assert_workspace_source_paths_are_stable(fragments: &[Fragment], expected_root: &Path) {\n    for fragment in fragments {\n        assert_eq!(\n            fragment.crate_root.as_deref(),\n            Some(expected_root),\n            \"crate reported an unexpected source root\",\n        );\n        for span in &fragment.spans {\n            assert!(\n                !span.file.contains('\\\\'),\n                \"span path keeps a native separator\",\n            );\n        }\n    }\n}\n";

    #[test]
    fn hawk_shaped_helper_is_assertion_shaped() -> Result<(), String> {
        let owner = function(
            "tests/fragments.rs",
            "assert_workspace_source_paths_are_stable",
            HAWK_SHAPED_BODY,
            false,
            Vec::new(),
        );
        let caller_test = function(
            "tests/fragments.rs",
            "workspace_paths_stay_stable",
            "fn workspace_paths_stay_stable() { assert_workspace_source_paths_are_stable(); }",
            true,
            vec!["assert_workspace_source_paths_are_stable"],
        );
        let index = RustIndex {
            functions: vec![owner.clone(), caller_test],
            ..RustIndex::default()
        };

        assert!(
            is_assertion_shaped_owner(&owner, &index),
            "test-called assertion helper must be assertion-shaped"
        );
        Ok(())
    }

    #[test]
    fn production_caller_blocks_assertion_shaped_detection() -> Result<(), String> {
        let owner = function(
            "src/lib.rs",
            "check_invariants",
            "fn check_invariants(value: i32) {\n    assert!(value >= 0);\n    assert_eq!(value % 2, 0);\n}\n",
            false,
            Vec::new(),
        );
        let production_caller = function(
            "src/lib.rs",
            "validate",
            "fn validate(value: i32) {\n    check_invariants(value);\n}\n",
            false,
            vec!["check_invariants"],
        );
        let index = RustIndex {
            functions: vec![owner.clone(), production_caller],
            ..RustIndex::default()
        };

        assert!(
            !is_assertion_shaped_owner(&owner, &index),
            "a non-test caller must keep the standard guidance"
        );
        Ok(())
    }

    #[test]
    fn non_assert_statement_blocks_dominance() -> Result<(), String> {
        let owner = function(
            "src/lib.rs",
            "load_config",
            "fn load_config(path: &Path) -> Config {\n    let text = fs::read_to_string(path).expect(\"config readable\");\n    parse_config(&text)\n}\n",
            false,
            Vec::new(),
        );
        let index = RustIndex {
            functions: vec![owner.clone()],
            ..RustIndex::default()
        };

        assert!(
            !is_assertion_shaped_owner(&owner, &index),
            "a tail `parse_config(&text)` expression is not an assertion"
        );
        Ok(())
    }

    #[test]
    fn no_assert_family_statement_blocks_dominance() -> Result<(), String> {
        let owner = function(
            "src/lib.rs",
            "normalize",
            "fn normalize(value: i32) -> i32 {\n    let clamped = value.max(0);\n    clamped\n}\n",
            false,
            Vec::new(),
        );
        let index = RustIndex {
            functions: vec![owner.clone()],
            ..RustIndex::default()
        };

        assert!(
            !is_assertion_shaped_owner(&owner, &index),
            "zero assert-family statements is never assertion-shaped"
        );
        Ok(())
    }

    #[test]
    fn comment_and_string_mentions_do_not_count_as_assertions() -> Result<(), String> {
        let owner = function(
            "src/lib.rs",
            "compute",
            "fn compute(value: i32) -> i32 {\n    // assert_eq!(value, 0);\n    let label = \"assert!(ready)\";\n    let _ = label;\n    value + 1\n}\n",
            false,
            Vec::new(),
        );
        let index = RustIndex {
            functions: vec![owner.clone()],
            ..RustIndex::default()
        };

        assert!(
            !is_assertion_shaped_owner(&owner, &index),
            "comment/string mentions of assert macros must not count"
        );
        Ok(())
    }

    #[test]
    fn word_boundary_keeps_my_assert_out_of_the_family() -> Result<(), String> {
        let owner = function(
            "src/lib.rs",
            "helper",
            "fn helper(value: i32) {\n    my_assert!(value > 0);\n}\n",
            false,
            Vec::new(),
        );
        let index = RustIndex {
            functions: vec![owner.clone()],
            ..RustIndex::default()
        };

        assert!(
            !is_assertion_shaped_owner(&owner, &index),
            "my_assert! is not the assert! macro"
        );
        Ok(())
    }

    #[test]
    fn test_annotated_caller_does_not_count_as_a_non_test_caller() -> Result<(), String> {
        let owner = function(
            "src/lib.rs",
            "assert_invariants",
            "fn assert_invariants(value: i32) {\n    assert!(value >= 0);\n    assert_eq!(value % 2, 0);\n}\n",
            false,
            Vec::new(),
        );
        let test_caller = function(
            "src/lib.rs",
            "invariants_hold",
            "fn invariants_hold() {\n    assert_invariants(2);\n}\n",
            true,
            vec!["assert_invariants"],
        );
        let index = RustIndex {
            functions: vec![owner.clone(), test_caller],
            ..RustIndex::default()
        };

        assert!(
            is_assertion_shaped_owner(&owner, &index),
            "a #[test] caller is not a production caller"
        );
        Ok(())
    }

    #[test]
    fn expect_in_let_binding_counts_as_assert_statement() -> Result<(), String> {
        let owner = function(
            "src/lib.rs",
            "assert_fragments_parse",
            "fn assert_fragments_parse(dir: &Path) {\n    for entry in fs::read_dir(dir).expect(\"fragments dir readable\") {\n        let path = entry.expect(\"entry readable\").path();\n        let text = fs::read_to_string(&path).expect(\"fragment readable\");\n        assert!(text.starts_with('{'));\n    }\n}\n",
            false,
            Vec::new(),
        );
        let index = RustIndex {
            functions: vec![owner.clone()],
            ..RustIndex::default()
        };

        assert!(
            is_assertion_shaped_owner(&owner, &index),
            "expect-based setup plus asserts stays assertion-shaped"
        );
        Ok(())
    }

    // Review finding (PR #2170): a single-line `if cond { side_effect(); }`
    // must not merge with the following line (which would classify the merge
    // as Assert) and must not hide the side effect inside the control header.
    #[test]
    fn control_prefix_boundary_does_not_ignore_lookalike_calls() -> Result<(), String> {
        // #2170 review: boundary-less "else"/"loop" prefixes would match
        // `elsewhere(...)` / `loopback(...)` and wrongly ignore real calls,
        // over-crediting dominance. Both lookalikes must block.
        for body in [
            "fn helper(v: i32) {\n    elsewhere(v);\n    assert!(v > 0);\n}\n",
            "fn helper(v: i32) {\n    loopback(v);\n    assert!(v > 0);\n}\n",
        ] {
            if body_is_assertion_dominated(body) {
                return Err(format!(
                    "a lookalike-prefixed call must block dominance: {body}"
                ));
            }
        }
        // Real control flow still reads as control flow.
        if !body_is_assertion_dominated(
            "fn helper(v: i32) {\n    if v > 0 {\n        assert!(v > 1);\n    } else {\n        assert!(v < 10);\n    }\n}\n",
        ) {
            return Err("real if/else control flow with asserts must stay dominated".to_string());
        }
        Ok(())
    }

    #[test]
    fn single_line_if_with_side_effect_blocks_dominance() -> Result<(), String> {
        let owner = function(
            "src/lib.rs",
            "helper",
            "fn helper(value: i32) {\n    if value > 0 { side_effect(); }\n    assert!(value > 1);\n}\n",
            false,
            Vec::new(),
        );
        let index = RustIndex {
            functions: vec![owner.clone()],
            ..RustIndex::default()
        };

        assert!(
            !is_assertion_shaped_owner(&owner, &index),
            "a side effect inside a single-line if must block the reframe"
        );
        Ok(())
    }

    #[test]
    fn single_line_if_wrapping_only_asserts_stays_assertion_shaped() -> Result<(), String> {
        let owner = function(
            "src/lib.rs",
            "assert_positive",
            "fn assert_positive(value: i32) {\n    if value == 0 { assert_ne!(value, 0); }\n    assert!(value > 0);\n}\n",
            false,
            Vec::new(),
        );
        let index = RustIndex {
            functions: vec![owner.clone()],
            ..RustIndex::default()
        };

        assert!(
            is_assertion_shaped_owner(&owner, &index),
            "a single-line if wrapping only asserts is still an oracle"
        );
        Ok(())
    }

    // Review finding (PR #2170): a caller located under `tests/` counts as a
    // test caller even when it is a plain helper fn without #[test]; a plain
    // helper in `src/` remains a production caller and blocks the reframe.
    #[test]
    fn plain_helper_caller_in_test_file_does_not_block_reframe() -> Result<(), String> {
        let owner = function(
            "src/lib.rs",
            "assert_invariants",
            "fn assert_invariants(value: i32) {\n    assert!(value >= 0);\n    assert_eq!(value % 2, 0);\n}\n",
            false,
            Vec::new(),
        );
        let test_helper_caller = function(
            "tests/helpers.rs",
            "check_defaults",
            "fn check_defaults() {\n    assert_invariants(2);\n}\n",
            false,
            vec!["assert_invariants"],
        );
        let index = RustIndex {
            functions: vec![owner.clone(), test_helper_caller],
            ..RustIndex::default()
        };

        assert!(
            is_assertion_shaped_owner(&owner, &index),
            "a plain helper under tests/ is a test caller"
        );
        Ok(())
    }

    #[test]
    fn bare_name_collision_with_same_named_function_blocks_reframe() -> Result<(), String> {
        // The spec's fail-closed claim (#2170 review): an oracle named
        // `check` and an unrelated same-named helper in another module are
        // indistinguishable by bare-name matching. The collision must BLOCK
        // the reframe — never reframe on a guessed identity.
        let owner = function(
            "src/invariants.rs",
            "check",
            "fn check(value: i32) {\n    assert!(value >= 0);\n    assert_eq!(value % 2, 0);\n}\n",
            false,
            Vec::new(),
        );
        // A production fn that calls `check(...)` — but its target is a
        // different same-named function (e.g. a local validation helper).
        let coincidental_caller = function(
            "src/handler.rs",
            "handle",
            "fn handle() {\n    check(2);\n}\n",
            false,
            vec!["check"],
        );
        let index = RustIndex {
            functions: vec![owner.clone(), coincidental_caller],
            ..RustIndex::default()
        };

        assert!(
            !is_assertion_shaped_owner(&owner, &index),
            "a bare-name collision must block the reframe (fail-closed)"
        );
        Ok(())
    }

    #[test]
    fn plain_helper_caller_in_src_blocks_reframe() -> Result<(), String> {
        let owner = function(
            "src/lib.rs",
            "assert_invariants",
            "fn assert_invariants(value: i32) {\n    assert!(value >= 0);\n    assert_eq!(value % 2, 0);\n}\n",
            false,
            Vec::new(),
        );
        let src_helper_caller = function(
            "src/util.rs",
            "check_defaults",
            "fn check_defaults() {\n    assert_invariants(2);\n}\n",
            false,
            vec!["assert_invariants"],
        );
        let index = RustIndex {
            functions: vec![owner.clone(), src_helper_caller],
            ..RustIndex::default()
        };

        assert!(
            !is_assertion_shaped_owner(&owner, &index),
            "a plain helper in src/ is a production caller"
        );
        Ok(())
    }
}
