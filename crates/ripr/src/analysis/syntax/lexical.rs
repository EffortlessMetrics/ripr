use crate::analysis::facts::{FileFacts, FunctionFact, TestFact};
use crate::analysis::rust_index::{
    extract_assertions, extract_call_facts, extract_literal_facts, extract_return_facts,
    is_test_file,
};
use crate::domain::SymbolId;
use std::path::{Path, PathBuf};

use super::{LexicalRustSyntaxAdapter, RustSyntaxAdapter, SyntaxNodeFact, TextRange};

impl RustSyntaxAdapter for LexicalRustSyntaxAdapter {
    fn summarize_file(&self, path: &Path, text: &str) -> Result<FileFacts, String> {
        Ok(summarize_file_lexically(
            path.to_path_buf(),
            text.to_string(),
        ))
    }

    fn changed_nodes(&self, facts: &FileFacts, ranges: &[TextRange]) -> Vec<SyntaxNodeFact> {
        owner_changed_nodes(facts, ranges)
    }
}

pub(crate) fn summarize_file_lexically(path: PathBuf, text: String) -> FileFacts {
    let source = text.clone();
    let lines: Vec<&str> = text.lines().collect();
    let mut functions = Vec::new();
    let mut tests = Vec::new();
    let mut file_calls = Vec::new();
    let mut file_returns = Vec::new();
    let mut file_literals = Vec::new();
    let mut pending_test = false;
    let mut i = 0usize;

    while i < lines.len() {
        let trimmed = lines[i].trim();
        if trimmed.starts_with("#[test]")
            || trimmed.starts_with("#[tokio::test]")
            || trimmed.starts_with("#[async_std::test]")
        {
            pending_test = true;
            i += 1;
            continue;
        }
        if pending_test && trimmed.starts_with("#[") {
            i += 1;
            continue;
        }
        if pending_test && trimmed.is_empty() {
            i += 1;
            continue;
        }

        if let Some(name) = function_name(trimmed) {
            let start_line = i + 1;
            let (end_line, body) = collect_function_body(&lines, i);
            let calls = extract_call_facts(&body, start_line);
            let returns = extract_return_facts(&body, start_line);
            let literals = extract_literal_facts(&body, start_line);
            file_calls.extend(calls.clone());
            file_returns.extend(returns.clone());
            file_literals.extend(literals.clone());
            let function = FunctionFact {
                id: SymbolId(format!("{}::{name}", path.display())),
                name: name.clone(),
                file: path.clone(),
                start_line,
                end_line,
                body: body.clone(),
                calls: calls.clone(),
                returns: returns.clone(),
                literals: literals.clone(),
                is_test: pending_test,
                // Lexical fallback path: no parser, no AST attrs
                // iterator, so attrs stay empty. Value-extraction-v2's
                // rstest support is parser-only.
                attrs: Vec::new(),
            };
            if pending_test || is_test_file(&path) {
                tests.push(TestFact {
                    name: name.clone(),
                    file: path.clone(),
                    start_line,
                    end_line,
                    body: body.clone(),
                    calls,
                    assertions: extract_assertions(&body, start_line),
                    literals,
                    attrs: Vec::new(),
                });
            }
            functions.push(function);
            pending_test = false;
            i = end_line;
            continue;
        }

        if !trimmed.is_empty() {
            pending_test = false;
        }
        i += 1;
    }

    file_calls.sort_by(|a, b| a.line.cmp(&b.line).then(a.name.cmp(&b.name)));
    file_calls.dedup_by(|a, b| a.line == b.line && a.name == b.name && a.text == b.text);
    file_returns.sort_by(|a, b| a.line.cmp(&b.line).then(a.text.cmp(&b.text)));
    file_returns.dedup_by(|a, b| a.line == b.line && a.text == b.text);
    file_literals.sort_by(|a, b| a.line.cmp(&b.line).then(a.value.cmp(&b.value)));
    file_literals.dedup_by(|a, b| a.line == b.line && a.value == b.value);

    FileFacts {
        path,
        functions,
        tests,
        calls: file_calls,
        returns: file_returns,
        literals: file_literals,
        probe_shapes: Vec::new(),
        source,
    }
}

fn owner_changed_nodes(facts: &FileFacts, ranges: &[TextRange]) -> Vec<SyntaxNodeFact> {
    let mut nodes = Vec::new();
    for range in ranges {
        let mut owners = facts
            .functions
            .iter()
            .filter(|function| {
                ranges_overlap(
                    range.start_line,
                    range.end_line,
                    function.start_line,
                    function.end_line,
                )
            })
            .collect::<Vec<_>>();
        owners.sort_by(|left, right| {
            function_span(left)
                .cmp(&function_span(right))
                .then(right.start_line.cmp(&left.start_line))
                .then(left.id.0.cmp(&right.id.0))
        });
        if let Some(function) = owners.first() {
            nodes.push(SyntaxNodeFact {
                file: function.file.clone(),
                kind: if function.is_test {
                    "test_function".to_string()
                } else {
                    "function".to_string()
                },
                start_line: function.start_line,
                end_line: function.end_line,
                text: function.body.clone(),
                owner: Some(function.id.clone()),
            });
        }
    }
    nodes.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.start_line.cmp(&right.start_line))
            .then(left.end_line.cmp(&right.end_line))
            .then(left.kind.cmp(&right.kind))
            .then(left.owner.cmp(&right.owner))
    });
    nodes.dedup_by(|left, right| {
        left.file == right.file
            && left.start_line == right.start_line
            && left.end_line == right.end_line
            && left.kind == right.kind
            && left.owner == right.owner
    });
    nodes
}

fn function_span(function: &FunctionFact) -> usize {
    function.end_line.saturating_sub(function.start_line)
}

fn ranges_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start <= right_end && right_start <= left_end
}

fn function_name(trimmed: &str) -> Option<String> {
    let mut cleaned = trimmed;
    if let Some(rest) = cleaned.strip_prefix("pub(crate) ") {
        cleaned = rest;
    } else if let Some(rest) = cleaned.strip_prefix("pub ") {
        cleaned = rest;
    }
    if let Some(rest) = cleaned.strip_prefix("async ") {
        cleaned = rest;
    }
    let cleaned = cleaned.strip_prefix("fn ")?;
    let mut name = String::new();
    for ch in cleaned.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            name.push(ch);
        } else {
            break;
        }
    }
    if name.is_empty() { None } else { Some(name) }
}

fn collect_function_body(lines: &[&str], start: usize) -> (usize, String) {
    let mut body = String::new();
    let mut depth = 0isize;
    let mut saw_open = false;
    let mut end = start + 1;

    for (idx, line) in lines.iter().enumerate().skip(start) {
        body.push_str(line);
        body.push('\n');
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
                saw_open = true;
            } else if ch == '}' {
                depth -= 1;
            }
        }
        end = idx + 1;
        if saw_open && depth <= 0 {
            break;
        }
    }

    (end, body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::OracleKind;

    #[test]
    fn lexical_adapter_summarizes_functions_tests_and_file_facts() -> Result<(), String> {
        let adapter = LexicalRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("src/lib.rs"),
            r#"
pub async fn load_value() -> Result<i32, String> {
    let value = helper(42);
    Ok(value)
}

#[test]
#[cfg(feature = "fast")]

fn checks_value() {
    let result = load_value();
    assert!(result.is_ok());
}
"#,
        )?;

        assert_eq!(facts.functions.len(), 2);
        assert!(
            facts
                .functions
                .iter()
                .any(|function| function.name == "load_value")
        );
        assert!(facts.tests.iter().any(|test| test.name == "checks_value"));
        assert!(facts.calls.iter().any(|call| call.name == "helper"));
        assert!(
            facts
                .returns
                .iter()
                .any(|ret| ret.text.contains("Ok(value)"))
        );
        assert!(facts.literals.iter().any(|literal| literal.value == "42"));
        assert!(
            facts
                .tests
                .iter()
                .flat_map(|test| test.assertions.iter())
                .any(|assertion| assertion.kind == OracleKind::SmokeOnly)
        );
        Ok(())
    }

    #[test]
    fn lexical_adapter_marks_functions_in_test_files_as_tests() -> Result<(), String> {
        let adapter = LexicalRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("tests/integration.rs"),
            r#"
fn integration_smoke() {
    run().unwrap();
}
"#,
        )?;

        assert_eq!(facts.functions.len(), 1);
        assert_eq!(facts.tests.len(), 1);
        assert_eq!(facts.tests[0].name, "integration_smoke");
        assert!(
            facts.tests[0]
                .assertions
                .iter()
                .any(|assertion| assertion.kind == OracleKind::SmokeOnly)
        );
        Ok(())
    }

    #[test]
    fn lexical_adapter_changed_nodes_deduplicates_overlapping_test_ranges() -> Result<(), String> {
        let adapter = LexicalRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("src/lib.rs"),
            r#"
#[test]
fn checks_value() {
    assert_eq!(1, 1);
}
"#,
        )?;

        let ranges = [
            TextRange {
                start_line: 3,
                start_column: 1,
                end_line: 3,
                end_column: 40,
            },
            TextRange {
                start_line: 4,
                start_column: 1,
                end_line: 4,
                end_column: 40,
            },
        ];
        let nodes = adapter.changed_nodes(&facts, &ranges);

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].kind, "test_function");
        assert_eq!(
            nodes[0].owner.as_ref().map(|owner| owner.0.as_str()),
            Some("src/lib.rs::checks_value")
        );
        Ok(())
    }
}
