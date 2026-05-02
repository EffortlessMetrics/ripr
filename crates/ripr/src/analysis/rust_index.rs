use crate::domain::{OracleStrength, SymbolId};
use ra_ap_syntax::{
    AstNode, Edition, SourceFile, TextSize,
    ast::{self, HasAttrs, HasName},
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default)]
pub struct RustIndex {
    pub files: BTreeMap<PathBuf, FileFacts>,
    pub tests: Vec<TestFact>,
    pub functions: Vec<FunctionFact>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileFacts {
    pub path: PathBuf,
    pub functions: Vec<FunctionFact>,
    pub tests: Vec<TestFact>,
    pub calls: Vec<CallFact>,
    pub returns: Vec<ReturnFact>,
    pub literals: Vec<LiteralFact>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FunctionFact {
    pub id: SymbolId,
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
    pub calls: Vec<CallFact>,
    pub returns: Vec<ReturnFact>,
    pub literals: Vec<LiteralFact>,
    pub is_test: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestFact {
    pub name: String,
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub body: String,
    pub calls: Vec<CallFact>,
    pub assertions: Vec<OracleFact>,
    pub literals: Vec<LiteralFact>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OracleFact {
    pub line: usize,
    pub text: String,
    pub strength: OracleStrength,
    pub observed_tokens: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallFact {
    pub line: usize,
    pub name: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReturnFact {
    pub line: usize,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiteralFact {
    pub line: usize,
    pub value: String,
}

pub type FunctionSummary = FunctionFact;
pub type TestSummary = TestFact;

pub trait RustSyntaxAdapter {
    fn summarize_file(&self, path: &Path, text: &str) -> Result<FileFacts, String>;

    fn changed_nodes(&self, facts: &FileFacts, ranges: &[TextRange]) -> Vec<SyntaxNodeFact>;
}

#[derive(Clone, Debug, Default)]
pub struct LexicalRustSyntaxAdapter;

#[derive(Clone, Debug, Default)]
pub struct RaRustSyntaxAdapter;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextRange {
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxNodeFact {
    pub file: PathBuf,
    pub kind: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    pub owner: Option<SymbolId>,
}

impl RustSyntaxAdapter for LexicalRustSyntaxAdapter {
    fn summarize_file(&self, path: &Path, text: &str) -> Result<FileFacts, String> {
        Ok(summarize_file_lexically(
            path.to_path_buf(),
            text.to_string(),
        ))
    }

    fn changed_nodes(&self, facts: &FileFacts, ranges: &[TextRange]) -> Vec<SyntaxNodeFact> {
        lexical_changed_nodes(facts, ranges)
    }
}

impl RustSyntaxAdapter for RaRustSyntaxAdapter {
    fn summarize_file(&self, path: &Path, text: &str) -> Result<FileFacts, String> {
        summarize_file_with_parser(path, text)
    }

    fn changed_nodes(&self, facts: &FileFacts, ranges: &[TextRange]) -> Vec<SyntaxNodeFact> {
        lexical_changed_nodes(facts, ranges)
    }
}

pub fn build_index(root: &Path, files: &[PathBuf]) -> Result<RustIndex, String> {
    let mut index = RustIndex::default();
    let adapter = RaRustSyntaxAdapter;
    let fallback = LexicalRustSyntaxAdapter;
    for file in files {
        let full = root.join(file);
        let text = std::fs::read_to_string(&full)
            .map_err(|err| format!("failed to read {}: {err}", full.display()))?;
        let summary = adapter
            .summarize_file(file, &text)
            .or_else(|_| fallback.summarize_file(file, &text))?;
        index.tests.extend(summary.tests.clone());
        index.functions.extend(summary.functions.clone());
        index.files.insert(file.clone(), summary);
    }
    Ok(index)
}

#[cfg(test)]
fn summarize_file(path: PathBuf, text: String) -> FileFacts {
    match RaRustSyntaxAdapter.summarize_file(&path, &text) {
        Ok(facts) => facts,
        Err(_) => summarize_file_lexically(path, text),
    }
}

fn summarize_file_lexically(path: PathBuf, text: String) -> FileFacts {
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
    }
}

fn summarize_file_with_parser(path: &Path, text: &str) -> Result<FileFacts, String> {
    let parse = SourceFile::parse(text, Edition::CURRENT);
    let errors = parse.errors();
    if !errors.is_empty() {
        return Err(format!("parser reported {} syntax errors", errors.len()));
    }

    let source = parse.tree();
    let line_index = LineIndex::new(text);
    let mut functions = Vec::new();
    let mut tests = Vec::new();
    let mut file_calls = Vec::new();
    let mut file_returns = Vec::new();
    let mut file_literals = Vec::new();
    let path_buf = path.to_path_buf();

    for function in source.syntax().descendants().filter_map(ast::Fn::cast) {
        let Some(name) = function.name().map(|name| name.text().to_string()) else {
            continue;
        };
        let fn_start = function
            .fn_token()
            .map(|token| token.text_range().start())
            .unwrap_or_else(|| function.syntax().text_range().start());
        let fn_end = function.syntax().text_range().end();
        let start_line = line_index.line(fn_start);
        let end_line = line_index.line_for_range_end(fn_end);
        let body = slice_text(text, fn_start, fn_end);
        let calls = extract_call_facts(&body, start_line);
        let returns = extract_return_facts(&body, start_line);
        let literals = extract_literal_facts(&body, start_line);
        let is_test = has_test_attribute(&function);

        file_calls.extend(calls.clone());
        file_returns.extend(returns.clone());
        file_literals.extend(literals.clone());

        let function_fact = FunctionFact {
            id: SymbolId(format!("{}::{name}", path.display())),
            name: name.clone(),
            file: path_buf.clone(),
            start_line,
            end_line,
            body: body.clone(),
            calls: calls.clone(),
            returns: returns.clone(),
            literals: literals.clone(),
            is_test,
        };

        if is_test || is_test_file(path) {
            tests.push(TestFact {
                name,
                file: path_buf.clone(),
                start_line,
                end_line,
                body,
                calls,
                assertions: extract_parser_oracles(&function, text, &line_index),
                literals,
            });
        }

        functions.push(function_fact);
    }

    file_calls.sort_by(|a, b| a.line.cmp(&b.line).then(a.name.cmp(&b.name)));
    file_calls.dedup_by(|a, b| a.line == b.line && a.name == b.name && a.text == b.text);
    file_returns.sort_by(|a, b| a.line.cmp(&b.line).then(a.text.cmp(&b.text)));
    file_returns.dedup_by(|a, b| a.line == b.line && a.text == b.text);
    file_literals.sort_by(|a, b| a.line.cmp(&b.line).then(a.value.cmp(&b.value)));
    file_literals.dedup_by(|a, b| a.line == b.line && a.value == b.value);

    Ok(FileFacts {
        path: path_buf,
        functions,
        tests,
        calls: file_calls,
        returns: file_returns,
        literals: file_literals,
    })
}

fn has_test_attribute(function: &ast::Fn) -> bool {
    function.attrs().any(|attr| {
        let compact = attr
            .syntax()
            .text()
            .to_string()
            .chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>();
        compact == "#[test]"
            || compact.starts_with("#[tokio::test")
            || compact.starts_with("#[async_std::test")
    })
}

fn extract_parser_oracles(
    function: &ast::Fn,
    text: &str,
    line_index: &LineIndex,
) -> Vec<OracleFact> {
    let mut assertions = Vec::new();
    for macro_call in function
        .syntax()
        .descendants()
        .filter_map(ast::MacroCall::cast)
    {
        let Some(path) = macro_call.path() else {
            continue;
        };
        let macro_name = path.syntax().text().to_string().replace(' ', "");
        if !is_assertion_macro(&macro_name) {
            continue;
        }
        let range = macro_call.syntax().text_range();
        let assertion_text = slice_macro_call_text(text, range.start(), range.end());
        assertions.push(OracleFact {
            line: line_index.line(range.start()),
            strength: classify_assertion(&assertion_text),
            observed_tokens: extract_identifier_tokens(&assertion_text),
            text: assertion_text,
        });
    }

    for method_call in function
        .syntax()
        .descendants()
        .filter_map(ast::MethodCallExpr::cast)
    {
        let Some(name) = method_call
            .name_ref()
            .map(|name| name.syntax().text().to_string())
        else {
            continue;
        };
        if name != "unwrap" && name != "expect" {
            continue;
        }
        let range = method_call.syntax().text_range();
        let text = slice_text(text, range.start(), range.end())
            .trim()
            .trim_end_matches(';')
            .to_string();
        assertions.push(OracleFact {
            line: line_index.line(range.start()),
            strength: OracleStrength::Smoke,
            observed_tokens: extract_identifier_tokens(&text),
            text,
        });
    }

    assertions.sort_by(|a, b| a.line.cmp(&b.line).then(a.text.cmp(&b.text)));
    assertions.dedup_by(|a, b| a.line == b.line && a.text == b.text);
    assertions
}

fn is_assertion_macro(macro_name: &str) -> bool {
    matches!(
        macro_name,
        "assert" | "assert_eq" | "assert_ne" | "assert_matches" | "matches"
    ) || macro_name.starts_with("insta::assert")
        || macro_name.contains("snapshot")
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

pub fn changed_nodes_for_lines(
    index: &RustIndex,
    file: &Path,
    lines: &[usize],
) -> Vec<SyntaxNodeFact> {
    let Some(facts) = index.files.get(file) else {
        return Vec::new();
    };
    let ranges = lines
        .iter()
        .map(|line| TextRange {
            start_line: *line,
            start_column: 1,
            end_line: *line,
            end_column: usize::MAX,
        })
        .collect::<Vec<_>>();
    LexicalRustSyntaxAdapter.changed_nodes(facts, &ranges)
}

fn lexical_changed_nodes(facts: &FileFacts, ranges: &[TextRange]) -> Vec<SyntaxNodeFact> {
    let mut nodes = Vec::new();
    for range in ranges {
        for function in &facts.functions {
            if ranges_overlap(
                range.start_line,
                range.end_line,
                function.start_line,
                function.end_line,
            ) {
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
    }
    nodes.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.start_line.cmp(&right.start_line))
            .then(left.kind.cmp(&right.kind))
    });
    nodes.dedup_by(|left, right| {
        left.file == right.file && left.start_line == right.start_line && left.kind == right.kind
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

fn is_test_file(path: &Path) -> bool {
    path.starts_with("tests")
        || path
            .to_string_lossy()
            .replace('\\', "/")
            .contains("/tests/")
}

#[derive(Clone, Debug)]
struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(text: &str) -> Self {
        let mut starts = vec![0];
        for (index, byte) in text.bytes().enumerate() {
            if byte == b'\n' {
                starts.push(index + 1);
            }
        }
        Self { starts }
    }

    fn line(&self, offset: TextSize) -> usize {
        self.line_from_offset(text_size_to_usize(offset))
    }

    fn line_for_range_end(&self, offset: TextSize) -> usize {
        self.line_from_offset(text_size_to_usize(offset).saturating_sub(1))
    }

    fn line_from_offset(&self, offset: usize) -> usize {
        match self.starts.binary_search(&offset) {
            Ok(index) => index + 1,
            Err(index) => index.max(1),
        }
    }
}

fn text_size_to_usize(offset: TextSize) -> usize {
    let value: u32 = offset.into();
    value as usize
}

fn slice_text(text: &str, start: TextSize, end: TextSize) -> String {
    let start = text_size_to_usize(start);
    let end = text_size_to_usize(end);
    text.get(start..end).unwrap_or("").to_string()
}

fn slice_macro_call_text(text: &str, start: TextSize, end: TextSize) -> String {
    let start = text_size_to_usize(start);
    let mut end = text_size_to_usize(end);
    let bytes = text.as_bytes();
    let mut cursor = end;
    while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'\n' {
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b';') {
        end = cursor + 1;
    }
    text.get(start..end).unwrap_or("").trim().to_string()
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
            let strength = classify_assertion(trimmed);
            out.push(OracleFact {
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

fn extract_call_facts(body: &str, start_line: usize) -> Vec<CallFact> {
    let mut calls = Vec::new();
    for (offset, line) in body.lines().enumerate() {
        let bytes = line.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'(' {
                let mut j = i;
                while j > 0 && (bytes[j - 1].is_ascii_alphanumeric() || bytes[j - 1] == b'_') {
                    j -= 1;
                }
                if j < i {
                    let name = &line[j..i];
                    if is_call_name(name) {
                        calls.push(CallFact {
                            line: start_line + offset,
                            name: name.to_string(),
                            text: line.trim().to_string(),
                        });
                    }
                }
            }
            i += 1;
        }
    }
    calls.sort_by(|a, b| a.line.cmp(&b.line).then(a.name.cmp(&b.name)));
    calls.dedup_by(|a, b| a.line == b.line && a.name == b.name && a.text == b.text);
    calls
}

fn is_call_name(name: &str) -> bool {
    !matches!(
        name,
        "if" | "while"
            | "match"
            | "for"
            | "loop"
            | "assert"
            | "assert_eq"
            | "assert_ne"
            | "assert_matches"
    )
}

fn extract_return_facts(body: &str, start_line: usize) -> Vec<ReturnFact> {
    let mut returns = Vec::new();
    for (offset, line) in body.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("return ")
            || trimmed.contains(" return ")
            || trimmed.contains("Ok(")
            || trimmed.contains("Err(")
            || trimmed.contains("Some(")
            || trimmed.contains("None")
        {
            returns.push(ReturnFact {
                line: start_line + offset,
                text: trimmed.to_string(),
            });
        }
    }
    returns.sort_by(|a, b| a.line.cmp(&b.line).then(a.text.cmp(&b.text)));
    returns.dedup_by(|a, b| a.line == b.line && a.text == b.text);
    returns
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
    let mut literals = extract_literal_facts(body, 1)
        .into_iter()
        .map(|literal| literal.value)
        .collect::<Vec<_>>();
    literals.sort();
    literals.dedup();
    literals
}

fn extract_literal_facts(body: &str, start_line: usize) -> Vec<LiteralFact> {
    let mut literals = Vec::new();
    for (offset, line) in body.lines().enumerate() {
        let mut current = String::new();
        for ch in line.chars() {
            if ch.is_ascii_digit() || (ch == '-' && current.is_empty()) {
                current.push(ch);
            } else if !current.is_empty() {
                if current != "-" {
                    literals.push(LiteralFact {
                        line: start_line + offset,
                        value: current.clone(),
                    });
                }
                current.clear();
            }
        }
        if !current.is_empty() && current != "-" {
            literals.push(LiteralFact {
                line: start_line + offset,
                value: current,
            });
        }
    }
    literals.sort_by(|a, b| a.line.cmp(&b.line).then(a.value.cmp(&b.value)));
    literals.dedup_by(|a, b| a.line == b.line && a.value == b.value);
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
    fn summarize_file_emits_file_facts() {
        let file = summarize_file(
            PathBuf::from("src/lib.rs"),
            r#"
pub fn parse(input: &str) -> Result<i32, Error> {
    if input == "42" {
        return Ok(42);
    }
    Err(Error::Bad)
}
"#
            .to_string(),
        );

        assert_eq!(file.path, PathBuf::from("src/lib.rs"));
        assert_eq!(file.functions.len(), 1);
        assert_eq!(file.functions[0].name, "parse");
        assert!(file.calls.iter().any(|call| call.name == "Ok"));
        assert!(file.returns.iter().any(|fact| fact.text.contains("Ok(42)")));
        assert!(file.literals.iter().any(|fact| fact.value == "42"));
    }

    #[test]
    fn lexical_adapter_exposes_syntax_boundary() -> Result<(), String> {
        let adapter = LexicalRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("src/lib.rs"),
            r#"
pub fn price(amount: i32) -> i32 {
    if amount > 10 { amount - 1 } else { amount }
}
"#,
        )?;
        let nodes = adapter.changed_nodes(
            &facts,
            &[TextRange {
                start_line: 3,
                start_column: 5,
                end_line: 3,
                end_column: 40,
            }],
        );

        assert_eq!(facts.functions.len(), 1);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].kind, "function");
        assert_eq!(
            nodes[0].owner.as_ref().map(|owner| owner.0.as_str()),
            Some("src/lib.rs::price")
        );
        Ok(())
    }

    #[test]
    fn parser_adapter_extracts_multiline_assertion_macro() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("tests/pricing.rs"),
            r#"
use fixture::discounted_total;

#[test]
#[cfg_attr(feature = "slow", ignore)]
fn exact_boundary_value_is_checked() {
    assert_eq!(
        discounted_total(100, 100),
        90
    );
}
"#,
        )?;

        assert_eq!(facts.tests.len(), 1);
        assert_eq!(facts.tests[0].name, "exact_boundary_value_is_checked");
        assert_eq!(facts.tests[0].start_line, 6);
        assert_eq!(facts.tests[0].assertions.len(), 1);
        assert_eq!(facts.tests[0].assertions[0].line, 7);
        assert_eq!(
            facts.tests[0].assertions[0].strength,
            OracleStrength::Strong
        );
        assert!(facts.tests[0].assertions[0].text.contains("assert_eq!"));
        assert!(
            facts.tests[0].assertions[0]
                .text
                .contains("discounted_total(100, 100)")
        );
        Ok(())
    }

    #[test]
    fn parser_adapter_treats_unwrap_and_expect_as_smoke_oracles() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let expect_call = format!(r#"    parse("").{}("parse succeeds");"#, "expect");
        let unwrap_call = format!(r#"    parse("42").{}();"#, "unwrap");
        let source = [
            "",
            "#[test]",
            "fn only_smoke_checks_error_path() {",
            expect_call.as_str(),
            unwrap_call.as_str(),
            "}",
            "",
        ]
        .join("\n");
        let facts = adapter.summarize_file(Path::new("tests/errors.rs"), &source)?;

        let assertions = &facts.tests[0].assertions;
        assert_eq!(assertions.len(), 2);
        assert_eq!(assertions[0].strength, OracleStrength::Smoke);
        assert_eq!(assertions[1].strength, OracleStrength::Smoke);
        assert!(
            assertions
                .iter()
                .any(|assertion| assertion.text.contains("expect"))
        );
        assert!(
            assertions
                .iter()
                .any(|assertion| assertion.text.contains("unwrap"))
        );
        Ok(())
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
