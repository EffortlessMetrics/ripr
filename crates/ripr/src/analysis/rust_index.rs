use crate::domain::{OracleStrength, SymbolId};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct RustIndex {
    pub files: BTreeMap<PathBuf, FileSummary>,
    pub tests: Vec<TestSummary>,
    pub functions: Vec<FunctionSummary>,
}

#[derive(Clone, Debug, Default)]
pub struct FileSummary {
    pub functions: Vec<FunctionSummary>,
    pub tests: Vec<TestSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionSummary {
    pub id: SymbolId,
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
    pub calls: Vec<String>,
    pub is_test: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestSummary {
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
    pub calls: Vec<String>,
    pub assertions: Vec<AssertionSummary>,
    pub literals: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssertionSummary {
    pub line: usize,
    pub text: String,
    pub strength: OracleStrength,
    pub observed_tokens: Vec<String>,
}

pub fn build_index(root: &Path, files: &[PathBuf]) -> Result<RustIndex, String> {
    let mut index = RustIndex::default();
    for file in files {
        let full = root.join(file);
        let text = std::fs::read_to_string(&full)
            .map_err(|err| format!("failed to read {}: {err}", full.display()))?;
        let summary = summarize_file(file.clone(), text);
        index.tests.extend(summary.tests.clone());
        index.functions.extend(summary.functions.clone());
        index.files.insert(file.clone(), summary);
    }
    Ok(index)
}

pub fn summarize_file(path: PathBuf, text: String) -> FileSummary {
    let lines: Vec<&str> = text.lines().collect();
    let mut functions = Vec::new();
    let mut tests = Vec::new();
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
            let calls = extract_calls(&body);
            let function = FunctionSummary {
                id: SymbolId(format!("{}::{name}", path.display())),
                name: name.clone(),
                file: path.clone(),
                start_line,
                end_line,
                body: body.clone(),
                calls: calls.clone(),
                is_test: pending_test,
            };
            if pending_test
                || path.starts_with("tests")
                || path.to_string_lossy().contains("/tests/")
            {
                tests.push(TestSummary {
                    name: name.clone(),
                    file: path.clone(),
                    start_line,
                    end_line,
                    body: body.clone(),
                    calls,
                    assertions: extract_assertions(&body, start_line),
                    literals: extract_literals(&body),
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

    FileSummary { functions, tests }
}

pub fn find_owner_function<'a>(
    index: &'a RustIndex,
    file: &Path,
    line: usize,
) -> Option<&'a FunctionSummary> {
    index.files.get(file).and_then(|summary| {
        summary
            .functions
            .iter()
            .filter(|f| f.start_line <= line && line <= f.end_line)
            .max_by_key(|f| f.start_line)
    })
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

fn extract_assertions(body: &str, start_line: usize) -> Vec<AssertionSummary> {
    let mut out = Vec::new();
    for (offset, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if is_assertion_line(trimmed) {
            let strength = classify_assertion(trimmed);
            out.push(AssertionSummary {
                line: start_line + offset,
                text: trimmed.to_string(),
                strength,
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
        || line.contains("insta::assert")
        || line.contains("snapshot!")
        || line.contains("expect_")
        || line.contains(".expect(")
        || line.contains(".unwrap(")
        || line.contains("should_panic")
}

fn classify_assertion(line: &str) -> OracleStrength {
    if line.contains("assert_eq!")
        || line.contains("assert_ne!")
        || line.contains("assert_matches!")
        || line.contains("matches!") && line.contains("Err(")
    {
        OracleStrength::Strong
    } else if line.contains("insta::assert") || line.contains("snapshot!") {
        OracleStrength::Medium
    } else if line.contains("is_ok")
        || line.contains("is_err")
        || line.contains("is_some")
        || line.contains("is_none")
        || line.contains(".unwrap(")
        || line.contains(".expect(")
    {
        OracleStrength::Smoke
    } else if line.contains("> 0")
        || line.contains("<")
        || line.contains(">")
        || line.contains("is_empty")
        || line.contains("contains")
        || line.contains("assert!")
    {
        OracleStrength::Weak
    } else {
        OracleStrength::Unknown
    }
}

pub fn extract_calls(body: &str) -> Vec<String> {
    let mut calls = Vec::new();
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'(' {
            let mut j = i;
            while j > 0 && (bytes[j - 1].is_ascii_alphanumeric() || bytes[j - 1] == b'_') {
                j -= 1;
            }
            if j < i {
                let name = &body[j..i];
                if !matches!(
                    name,
                    "if" | "while"
                        | "match"
                        | "for"
                        | "loop"
                        | "assert"
                        | "assert_eq"
                        | "assert_ne"
                        | "assert_matches"
                ) {
                    calls.push(name.to_string());
                }
            }
        }
        i += 1;
    }
    calls.sort();
    calls.dedup();
    calls
}

pub fn extract_identifier_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            if is_interesting_token(&current) {
                tokens.push(current.clone());
            }
            current.clear();
        }
    }
    if is_interesting_token(&current) {
        tokens.push(current);
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

fn is_interesting_token(token: &str) -> bool {
    token.len() > 2
        && !matches!(
            token,
            "assert"
                | "assert_eq"
                | "assert_ne"
                | "assert_matches"
                | "let"
                | "mut"
                | "true"
                | "false"
                | "Some"
                | "None"
                | "Ok"
                | "Err"
                | "unwrap"
                | "expect"
                | "is_ok"
                | "is_err"
        )
}

pub fn extract_literals(body: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut current = String::new();
    for ch in body.chars() {
        if ch.is_ascii_digit() || (ch == '-' && current.is_empty()) {
            current.push(ch);
        } else if !current.is_empty() {
            if current != "-" {
                literals.push(current.clone());
            }
            current.clear();
        }
    }
    if !current.is_empty() && current != "-" {
        literals.push(current);
    }
    literals.sort();
    literals.dedup();
    literals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_tests_and_assertions() {
        let file = summarize_file(
            PathBuf::from("src/lib.rs"),
            r#"
#[test]
fn checks_error() {
    let result = parse("x");
    assert!(result.is_err());
}
"#
            .to_string(),
        );
        assert_eq!(file.tests.len(), 1);
        assert_eq!(file.tests[0].assertions.len(), 1);
        assert_eq!(file.tests[0].assertions[0].strength, OracleStrength::Smoke);
    }

    #[test]
    fn preserves_test_marker_across_stacked_attributes() {
        let file = summarize_file(
            PathBuf::from("src/lib.rs"),
            r#"
#[test]
#[should_panic]
fn panics_on_bad_input() {}

#[test]
#[ignore]
fn slow_but_real_test() {}

#[test]
#[cfg(feature = "foo")]
fn feature_gated_test() {}
"#
            .to_string(),
        );
        let names = file
            .tests
            .iter()
            .map(|test| test.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec![
                "panics_on_bad_input",
                "slow_but_real_test",
                "feature_gated_test"
            ]
        );
    }
}
