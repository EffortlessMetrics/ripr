use crate::domain::SymbolId;
use std::path::{Path, PathBuf};

use super::super::facts::FileFacts;
use super::{LexicalRustSyntaxAdapter, RustSyntaxAdapter, SyntaxNodeFact, TextRange};
use crate::analysis::rust_index::{
    FunctionFact, OracleFact, TestFact, classify_assertion, extract_call_facts,
    extract_identifier_tokens, extract_literal_facts, extract_return_facts, is_test_file,
};

impl RustSyntaxAdapter for LexicalRustSyntaxAdapter {
    fn summarize_file(&self, path: &Path, text: &str) -> Result<FileFacts, String> {
        Ok(summarize_file_lexically(path.to_path_buf(), text))
    }

    fn changed_nodes(&self, facts: &FileFacts, ranges: &[TextRange]) -> Vec<SyntaxNodeFact> {
        owner_changed_nodes(facts, ranges)
    }
}

pub fn summarize_file_lexically(path: PathBuf, text: &str) -> FileFacts {
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
                });
            }
            functions.push(function);
            pending_test = false;
            i = end_line;
            continue;
        }

        if !trimmed.is_empty() && !trimmed.starts_with("//") {
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
    }
}

fn function_name(trimmed: &str) -> Option<String> {
    let mut cleaned = trimmed;

    // Strip visibility modifiers
    if let Some(rest) = cleaned.strip_prefix("pub(crate) ") {
        cleaned = rest;
    } else if cleaned.starts_with("pub(") {
        if let Some(end_idx) = cleaned.find(')') {
            cleaned = &cleaned[end_idx + 1..].trim_start();
        }
    } else if let Some(rest) = cleaned.strip_prefix("pub ") {
        cleaned = rest;
    }

    // Strip modifiers (may have multiple like "async unsafe")
    loop {
        if let Some(rest) = cleaned.strip_prefix("async ") {
            cleaned = rest;
        } else if let Some(rest) = cleaned.strip_prefix("unsafe ") {
            cleaned = rest;
        } else if let Some(rest) = cleaned.strip_prefix("const ") {
            cleaned = rest;
        } else if let Some(rest) = cleaned.strip_prefix("extern ") {
            cleaned = rest;
        } else {
            break;
        }
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

fn extract_assertions(body: &str, start_line: usize) -> Vec<OracleFact> {
    let mut out = Vec::new();
    for (offset, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if is_assertion_line(trimmed) {
            let classification = classify_assertion(trimmed);
            out.push(OracleFact {
                line: start_line + offset,
                text: trimmed.to_string(),
                kind: classification.kind,
                strength: classification.strength,
                observed_tokens: extract_identifier_tokens(trimmed),
            });
        }
    }
    out
}

fn is_assertion_line(line: &str) -> bool {
    line.contains("assert!")
        || line.contains("assert_eq!")
        || line.contains("assert_ne!")
        || line.contains("assert_matches!")
        || line.contains("matches!")
        || is_snapshot_assertion(line)
        || line.contains("expect_")
        || line.contains(".expect(")
        || line.contains(".unwrap(")
        || line.contains("should_panic")
}

fn is_snapshot_assertion(line: &str) -> bool {
    let expect_test_comparison = (line.contains("expect![[") || line.contains("expect_file!["))
        && (line.contains(".assert_eq(")
            || line.contains(".assert_debug_eq(")
            || line.contains(".assert_json_eq("));
    let known_snapshot_macros = [
        "assert_snapshot!",
        "assert_yaml_snapshot!",
        "assert_json_snapshot!",
        "assert_debug_snapshot!",
        "assert_display_snapshot!",
        "assert_csv_snapshot!",
        "assert_ron_snapshot!",
        "assert_toml_snapshot!",
    ];
    known_snapshot_macros
        .iter()
        .any(|macro_name| line.contains(macro_name))
        || expect_test_comparison
}

fn owner_changed_nodes(
    facts: &crate::analysis::facts::FileFacts,
    ranges: &[TextRange],
) -> Vec<SyntaxNodeFact> {
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

fn ranges_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start <= right_end && right_start <= left_end
}

fn function_span(function: &crate::analysis::rust_index::FunctionFact) -> usize {
    function.end_line.saturating_sub(function.start_line)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-{name}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn lexical_adapter_parses_simple_functions() {
        let root = temp_dir("lexical_simple");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='test'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[test]
fn test_add() {
    assert_eq!(add(1, 2), 3);
}
"#,
        )
        .unwrap();

        let adapter = LexicalRustSyntaxAdapter;
        let text = fs::read_to_string(root.join("src/lib.rs")).unwrap();
        let result = adapter.summarize_file(&root.join("src/lib.rs"), &text);

        assert!(result.is_ok());
        let facts = result.unwrap();
        assert!(!facts.functions.is_empty());
        assert!(!facts.tests.is_empty());
    }

    #[test]
    fn lexical_adapter_extracts_assertions() {
        let root = temp_dir("lexical_assertions");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='test'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            r#"
#[test]
fn test_math() {
    let result = 2 + 2;
    assert_eq!(result, 4);
    assert!(result > 0);
}
"#,
        )
        .unwrap();

        let adapter = LexicalRustSyntaxAdapter;
        let text = fs::read_to_string(root.join("src/lib.rs")).unwrap();
        let facts = adapter
            .summarize_file(&root.join("src/lib.rs"), &text)
            .unwrap();

        assert!(!facts.tests.is_empty());
        let test = &facts.tests[0];
        assert!(!test.assertions.is_empty());
    }

    #[test]
    fn lexical_adapter_handles_async_tests() {
        let root = temp_dir("lexical_async");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='test'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            r#"
#[tokio::test]
async fn test_async_func() {
    let result = 42;
    assert!(result > 0);
}

#[async_std::test]
async fn test_async_std_func() {
    assert_eq!(1 + 1, 2);
}
"#,
        )
        .unwrap();

        let adapter = LexicalRustSyntaxAdapter;
        let text = fs::read_to_string(root.join("src/lib.rs")).unwrap();
        let facts = adapter
            .summarize_file(&root.join("src/lib.rs"), &text)
            .unwrap();

        assert_eq!(facts.tests.len(), 2);
        assert!(facts.tests[0].name.contains("async"));
        assert!(facts.tests[1].name.contains("async"));
    }

    #[test]
    fn lexical_adapter_changed_nodes_identifies_owners() {
        let root = temp_dir("lexical_owners");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='test'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn outer() {
    fn inner() {
        let x = 1;
    }
}
"#,
        )
        .unwrap();

        let adapter = LexicalRustSyntaxAdapter;
        let text = fs::read_to_string(root.join("src/lib.rs")).unwrap();
        let facts = adapter
            .summarize_file(&root.join("src/lib.rs"), &text)
            .unwrap();

        let ranges = vec![TextRange {
            start_line: 3,
            start_column: 1,
            end_line: 5,
            end_column: 10,
        }];

        let nodes = adapter.changed_nodes(&facts, &ranges);
        assert!(!nodes.is_empty());
        assert!(nodes[0].owner.is_some());
    }
}
