use crate::domain::{OracleStrength, SymbolId};
use ra_ap_syntax::{
    AstNode, Edition, SourceFile, TextSize,
    ast::{self, HasAttrs, HasName},
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const PROBE_SHAPE_PREDICATE: &str = "predicate";
pub const PROBE_SHAPE_RETURN_VALUE: &str = "return_value";
pub const PROBE_SHAPE_ERROR_PATH: &str = "error_path";
pub const PROBE_SHAPE_CALL_DELETION: &str = "call_deletion";
pub const PROBE_SHAPE_FIELD_CONSTRUCTION: &str = "field_construction";
pub const PROBE_SHAPE_SIDE_EFFECT: &str = "side_effect";
pub const PROBE_SHAPE_MATCH_ARM: &str = "match_arm";

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
    pub probe_shapes: Vec<ProbeShapeFact>,
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeShapeFact {
    pub start_line: usize,
    pub end_line: usize,
    pub kind: String,
    pub text: String,
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
        owner_changed_nodes(facts, ranges)
    }
}

impl RustSyntaxAdapter for RaRustSyntaxAdapter {
    fn summarize_file(&self, path: &Path, text: &str) -> Result<FileFacts, String> {
        summarize_file_with_parser(path, text)
    }

    fn changed_nodes(&self, facts: &FileFacts, ranges: &[TextRange]) -> Vec<SyntaxNodeFact> {
        owner_changed_nodes(facts, ranges)
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
        probe_shapes: Vec::new(),
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
    let mut file_probe_shapes = Vec::new();
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
        let probe_shapes = extract_parser_probe_shapes(&function, text, &line_index);
        let is_test = has_test_attribute(&function);

        file_calls.extend(calls.clone());
        file_returns.extend(returns.clone());
        file_literals.extend(literals.clone());
        file_probe_shapes.extend(probe_shapes);

        let function_fact = FunctionFact {
            id: parser_symbol_id(path, &function, &name),
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

    disambiguate_duplicate_symbol_ids(&mut functions);

    file_calls.sort_by(|a, b| a.line.cmp(&b.line).then(a.name.cmp(&b.name)));
    file_calls.dedup_by(|a, b| a.line == b.line && a.name == b.name && a.text == b.text);
    file_returns.sort_by(|a, b| a.line.cmp(&b.line).then(a.text.cmp(&b.text)));
    file_returns.dedup_by(|a, b| a.line == b.line && a.text == b.text);
    file_literals.sort_by(|a, b| a.line.cmp(&b.line).then(a.value.cmp(&b.value)));
    file_literals.dedup_by(|a, b| a.line == b.line && a.value == b.value);
    file_probe_shapes.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.end_line.cmp(&b.end_line))
            .then(a.kind.cmp(&b.kind))
            .then(a.text.cmp(&b.text))
    });
    file_probe_shapes.dedup_by(|a, b| {
        a.start_line == b.start_line
            && a.end_line == b.end_line
            && a.kind == b.kind
            && a.text == b.text
    });

    Ok(FileFacts {
        path: path_buf,
        functions,
        tests,
        calls: file_calls,
        returns: file_returns,
        literals: file_literals,
        probe_shapes: file_probe_shapes,
    })
}

fn parser_symbol_id(path: &Path, function: &ast::Fn, name: &str) -> SymbolId {
    let mut segments = vec![path.display().to_string()];

    let mut modules = function
        .syntax()
        .ancestors()
        .skip(1)
        .filter_map(ast::Module::cast)
        .filter_map(|module| {
            module
                .name()
                .map(|module_name| module_name.text().to_string())
        })
        .collect::<Vec<_>>();
    modules.reverse();
    segments.extend(modules);

    if let Some(impl_block) = function
        .syntax()
        .ancestors()
        .skip(1)
        .find_map(ast::Impl::cast)
    {
        segments.push(impl_owner_segment(&impl_block));
    }

    segments.push(name.to_string());
    SymbolId(segments.join("::"))
}

fn impl_owner_segment(impl_block: &ast::Impl) -> String {
    let self_ty = match impl_block.self_ty() {
        Some(ty) => compact_syntax_text(ty.syntax().text().to_string()),
        None => "unknown".to_string(),
    };
    match impl_block.trait_() {
        Some(trait_ty) => format!(
            "impl {} for {self_ty}",
            compact_syntax_text(trait_ty.syntax().text().to_string())
        ),
        None => format!("impl {self_ty}"),
    }
}

fn compact_syntax_text(text: String) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn disambiguate_duplicate_symbol_ids(functions: &mut [FunctionFact]) {
    let mut totals = BTreeMap::new();
    for function in functions.iter() {
        let entry = totals.entry(function.id.0.clone()).or_insert(0usize);
        *entry += 1;
    }

    for function in functions.iter_mut() {
        let total = match totals.get(&function.id.0) {
            Some(total) => *total,
            None => 0,
        };
        if total > 1 {
            function.id.0 = format!("{}#L{}", function.id.0, function.start_line);
        }
    }
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

fn extract_parser_probe_shapes(
    function: &ast::Fn,
    text: &str,
    line_index: &LineIndex,
) -> Vec<ProbeShapeFact> {
    let mut shapes = Vec::new();
    for if_expr in function
        .syntax()
        .descendants()
        .filter_map(ast::IfExpr::cast)
    {
        if let Some(condition) = if_expr.condition() {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_PREDICATE,
                condition.syntax().text_range().start(),
                condition.syntax().text_range().end(),
            );
        }
    }

    for while_expr in function
        .syntax()
        .descendants()
        .filter_map(ast::WhileExpr::cast)
    {
        if let Some(condition) = while_expr.condition() {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_PREDICATE,
                condition.syntax().text_range().start(),
                condition.syntax().text_range().end(),
            );
        }
    }

    for bin_expr in function
        .syntax()
        .descendants()
        .filter_map(ast::BinExpr::cast)
    {
        if bin_expr
            .op_token()
            .is_some_and(|token| is_predicate_operator(token.text()))
        {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_PREDICATE,
                bin_expr.syntax().text_range().start(),
                bin_expr.syntax().text_range().end(),
            );
        }
    }

    for return_expr in function
        .syntax()
        .descendants()
        .filter_map(ast::ReturnExpr::cast)
    {
        let range = return_expr.syntax().text_range();
        push_probe_shape(
            &mut shapes,
            line_index,
            text,
            PROBE_SHAPE_RETURN_VALUE,
            range.start(),
            range.end(),
        );
        let return_text = slice_text(text, range.start(), range.end());
        if has_error_path_text(&return_text) {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_ERROR_PATH,
                range.start(),
                range.end(),
            );
        }
    }

    if let Some(tail_expr) = function.body().and_then(|body| body.tail_expr()) {
        let range = tail_expr.syntax().text_range();
        let tail_text = slice_text(text, range.start(), range.end());
        if is_tail_return_value_text(&tail_text) {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_RETURN_VALUE,
                range.start(),
                range.end(),
            );
            if has_error_path_text(&tail_text) {
                push_probe_shape(
                    &mut shapes,
                    line_index,
                    text,
                    PROBE_SHAPE_ERROR_PATH,
                    range.start(),
                    range.end(),
                );
            }
        }
    }

    for call_expr in function
        .syntax()
        .descendants()
        .filter_map(ast::CallExpr::cast)
    {
        let range = call_expr.syntax().text_range();
        let call_text = slice_text(text, range.start(), range.end());
        push_probe_shape(
            &mut shapes,
            line_index,
            text,
            PROBE_SHAPE_CALL_DELETION,
            range.start(),
            range.end(),
        );
        if has_return_value_text(&call_text) {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_RETURN_VALUE,
                range.start(),
                range.end(),
            );
        }
        if has_error_path_text(&call_text) {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_ERROR_PATH,
                range.start(),
                range.end(),
            );
        }
    }

    for method_call in function
        .syntax()
        .descendants()
        .filter_map(ast::MethodCallExpr::cast)
    {
        let range = method_call.syntax().text_range();
        let method_text = slice_text(text, range.start(), range.end());
        push_probe_shape(
            &mut shapes,
            line_index,
            text,
            PROBE_SHAPE_CALL_DELETION,
            range.start(),
            range.end(),
        );
        if method_call
            .name_ref()
            .is_some_and(|name| is_effect_call_name(&name.syntax().text().to_string()))
            || has_effect_text(&method_text)
        {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_SIDE_EFFECT,
                range.start(),
                range.end(),
            );
        }
    }

    for field in function
        .syntax()
        .descendants()
        .filter_map(ast::RecordExprField::cast)
    {
        let range = field.syntax().text_range();
        push_probe_shape(
            &mut shapes,
            line_index,
            text,
            PROBE_SHAPE_FIELD_CONSTRUCTION,
            range.start(),
            range.end(),
        );
    }

    for match_expr in function
        .syntax()
        .descendants()
        .filter_map(ast::MatchExpr::cast)
    {
        if let Some(token) = match_expr.match_token() {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_MATCH_ARM,
                token.text_range().start(),
                token.text_range().end(),
            );
        }
    }

    for arm in function
        .syntax()
        .descendants()
        .filter_map(ast::MatchArm::cast)
    {
        if let Some(token) = arm.fat_arrow_token() {
            push_probe_shape(
                &mut shapes,
                line_index,
                text,
                PROBE_SHAPE_MATCH_ARM,
                token.text_range().start(),
                token.text_range().end(),
            );
        }
    }

    shapes.sort_by(|a, b| {
        a.start_line
            .cmp(&b.start_line)
            .then(a.end_line.cmp(&b.end_line))
            .then(a.kind.cmp(&b.kind))
            .then(a.text.cmp(&b.text))
    });
    shapes.dedup_by(|a, b| {
        a.start_line == b.start_line
            && a.end_line == b.end_line
            && a.kind == b.kind
            && a.text == b.text
    });
    shapes
}

fn push_probe_shape(
    shapes: &mut Vec<ProbeShapeFact>,
    line_index: &LineIndex,
    text: &str,
    kind: &str,
    start: TextSize,
    end: TextSize,
) {
    let snippet = slice_text(text, start, end)
        .trim()
        .trim_end_matches(';')
        .to_string();
    if snippet.is_empty() {
        return;
    }
    shapes.push(ProbeShapeFact {
        start_line: line_index.line(start),
        end_line: line_index.line_for_range_end(end),
        kind: kind.to_string(),
        text: snippet,
    });
}

fn is_predicate_operator(operator: &str) -> bool {
    matches!(
        operator,
        "==" | "!=" | "<=" | ">=" | "<" | ">" | "&&" | "||"
    )
}

fn has_return_value_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    trimmed.starts_with("Ok(")
        || trimmed.starts_with("Some(")
        || trimmed.contains(" Ok(")
        || trimmed.contains(" Some(")
        || trimmed.contains("None")
}

fn is_tail_return_value_text(text: &str) -> bool {
    let trimmed = text.trim_start();
    !trimmed.is_empty()
        && !trimmed.starts_with("if ")
        && !trimmed.starts_with("match ")
        && !trimmed.starts_with("while ")
        && !trimmed.starts_with("for ")
        && !trimmed.starts_with("loop ")
}

fn has_error_path_text(text: &str) -> bool {
    text.contains("Err(")
        || text.contains("Error::")
        || text.contains("map_err")
        || text.contains("bail!")
        || text.contains("anyhow!")
}

fn has_effect_text(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        ".save(",
        ".publish(",
        ".send(",
        ".write(",
        ".insert(",
        ".push(",
        ".remove(",
        ".delete(",
        ".emit(",
        ".increment(",
        "metrics.",
        "log::",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn is_effect_call_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "save"
            | "publish"
            | "send"
            | "write"
            | "insert"
            | "push"
            | "remove"
            | "delete"
            | "emit"
            | "increment"
    )
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
    RaRustSyntaxAdapter.changed_nodes(facts, &ranges)
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
        assert!(
            file.probe_shapes
                .iter()
                .any(|shape| shape.kind == PROBE_SHAPE_RETURN_VALUE)
        );
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
    fn parser_owner_symbols_include_module_paths() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("src/lib.rs"),
            r#"
mod pricing {
    pub fn score(amount: i32) -> i32 {
        amount + 1
    }
}

mod reporting {
    pub fn score(amount: i32) -> i32 {
        amount + 2
    }
}
"#,
        )?;
        let ids = facts
            .functions
            .iter()
            .map(|function| function.id.0.as_str())
            .collect::<Vec<_>>();

        assert!(ids.contains(&"src/lib.rs::pricing::score"));
        assert!(ids.contains(&"src/lib.rs::reporting::score"));
        Ok(())
    }

    #[test]
    fn parser_owner_symbols_include_impl_targets() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("src/lib.rs"),
            r#"
struct Discount;

impl Discount {
    pub fn score(&self, amount: i32) -> i32 {
        amount + 1
    }
}

struct Tax;

impl Tax {
    pub fn score(&self, amount: i32) -> i32 {
        amount + 2
    }
}
"#,
        )?;
        let ids = facts
            .functions
            .iter()
            .map(|function| function.id.0.as_str())
            .collect::<Vec<_>>();

        assert!(ids.contains(&"src/lib.rs::impl Discount::score"));
        assert!(ids.contains(&"src/lib.rs::impl Tax::score"));
        Ok(())
    }

    #[test]
    fn changed_nodes_use_module_qualified_owner() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let source = r#"
mod pricing {
    pub fn score(amount: i32) -> i32 {
        if amount >= 100 { 90 } else { 100 }
    }
}

mod reporting {
    pub fn score(amount: i32) -> i32 {
        amount + 2
    }
}
"#;
        let facts = adapter.summarize_file(Path::new("src/lib.rs"), source)?;
        let changed_line = line_containing(source, "amount >= 100")?;
        let mut index = RustIndex::default();
        index.files.insert(PathBuf::from("src/lib.rs"), facts);
        let nodes = changed_nodes_for_lines(&index, Path::new("src/lib.rs"), &[changed_line]);

        assert_eq!(nodes.len(), 1);
        assert_eq!(
            nodes[0].owner.as_ref().map(|owner| owner.0.as_str()),
            Some("src/lib.rs::pricing::score")
        );
        Ok(())
    }

    #[test]
    fn changed_nodes_preserve_test_owner_under_cfg_module() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let source = r#"
#[cfg(test)]
mod tests {
    #[test]
    fn checks_boundary() {
        assert_eq!(discounted_total(100), 90);
    }
}
"#;
        let facts = adapter.summarize_file(Path::new("src/lib.rs"), source)?;
        let changed_line = line_containing(source, "discounted_total")?;
        let mut index = RustIndex::default();
        index.files.insert(PathBuf::from("src/lib.rs"), facts);
        let nodes = changed_nodes_for_lines(&index, Path::new("src/lib.rs"), &[changed_line]);

        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].kind, "test_function");
        assert_eq!(
            nodes[0].owner.as_ref().map(|owner| owner.0.as_str()),
            Some("src/lib.rs::tests::checks_boundary")
        );
        Ok(())
    }

    #[test]
    fn parser_adapter_extracts_probe_shapes_from_syntax() -> Result<(), String> {
        let adapter = RaRustSyntaxAdapter;
        let facts = adapter.summarize_file(
            Path::new("src/lib.rs"),
            r#"
pub fn classify(amount: i32, service: &mut Service) -> Result<Quote, Error> {
    if amount >= 100 {
        service.publish(
            Event::Discounted,
        );
        return Ok(Quote {
            total: 90,
        });
    }

    match amount {
        0 => Err(Error::Zero),
        _ => Ok(Quote { total: amount }),
    }
}
"#,
        )?;
        let kinds = facts
            .probe_shapes
            .iter()
            .map(|shape| shape.kind.as_str())
            .collect::<Vec<_>>();

        assert!(kinds.contains(&PROBE_SHAPE_PREDICATE));
        assert!(kinds.contains(&PROBE_SHAPE_RETURN_VALUE));
        assert!(kinds.contains(&PROBE_SHAPE_ERROR_PATH));
        assert!(kinds.contains(&PROBE_SHAPE_CALL_DELETION));
        assert!(kinds.contains(&PROBE_SHAPE_FIELD_CONSTRUCTION));
        assert!(kinds.contains(&PROBE_SHAPE_SIDE_EFFECT));
        assert!(kinds.contains(&PROBE_SHAPE_MATCH_ARM));
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

    fn line_containing(source: &str, needle: &str) -> Result<usize, String> {
        match source.lines().position(|line| line.contains(needle)) {
            Some(index) => Ok(index + 1),
            None => Err(format!("missing line containing {needle}")),
        }
    }
}
