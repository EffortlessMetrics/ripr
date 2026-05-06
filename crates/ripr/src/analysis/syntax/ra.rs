use crate::domain::{OracleKind, OracleStrength, SymbolId};
use ra_ap_syntax::{
    AstNode, Edition, SourceFile, TextSize,
    ast::{self, HasAttrs, HasName},
};
use std::collections::BTreeMap;
use std::path::Path;

use super::super::facts::FileFacts;
use super::{RaRustSyntaxAdapter, RustSyntaxAdapter, SyntaxNodeFact, TextRange};
use crate::analysis::rust_index::{
    FunctionFact, OracleFact, PROBE_SHAPE_CALL_DELETION, PROBE_SHAPE_ERROR_PATH,
    PROBE_SHAPE_FIELD_CONSTRUCTION, PROBE_SHAPE_MATCH_ARM, PROBE_SHAPE_PREDICATE,
    PROBE_SHAPE_RETURN_VALUE, PROBE_SHAPE_SIDE_EFFECT, ProbeShapeFact, extract_identifier_tokens,
};

impl RustSyntaxAdapter for RaRustSyntaxAdapter {
    fn summarize_file(&self, path: &Path, text: &str) -> Result<FileFacts, String> {
        summarize_file_with_parser(path, text)
    }

    fn changed_nodes(&self, facts: &FileFacts, ranges: &[TextRange]) -> Vec<SyntaxNodeFact> {
        owner_changed_nodes(facts, ranges)
    }
}

pub fn summarize_file_with_parser(path: &Path, text: &str) -> Result<FileFacts, String> {
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
        let calls = crate::analysis::rust_index::extract_call_facts(&body, start_line);
        let returns = crate::analysis::rust_index::extract_return_facts(&body, start_line);
        let literals = crate::analysis::rust_index::extract_literal_facts(&body, start_line);
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

        if is_test || crate::analysis::rust_index::is_test_file(path) {
            tests.push(crate::analysis::rust_index::TestFact {
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
        start_byte: u32::from(start) as usize,
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
        let classification = crate::analysis::rust_index::classify_assertion(&assertion_text);
        assertions.push(OracleFact {
            line: line_index.line(range.start()),
            kind: classification.kind,
            strength: classification.strength,
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
            kind: OracleKind::SmokeOnly,
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

fn ranges_overlap(start1: usize, end1: usize, start2: usize, end2: usize) -> bool {
    start1 <= end2 && start2 <= end1
}

fn function_span(function: &crate::analysis::rust_index::FunctionFact) -> usize {
    function.end_line - function.start_line
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ripr-{name}-{stamp}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn ra_adapter_parses_valid_rust_source() {
        let root = temp_dir("ra_parser");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='test'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn calculate(x: i32, y: i32) -> i32 {
    if x > y { x } else { y }
}

#[test]
fn test_calculate() {
    assert_eq!(calculate(5, 3), 5);
}
"#,
        )
        .unwrap();

        let adapter = RaRustSyntaxAdapter;
        let text = fs::read_to_string(root.join("src/lib.rs")).unwrap();
        let result = adapter.summarize_file(&root.join("src/lib.rs"), &text);

        assert!(result.is_ok());
        let facts = result.unwrap();
        assert!(!facts.functions.is_empty());
        assert!(!facts.tests.is_empty());
        assert!(!facts.probe_shapes.is_empty());
    }

    #[test]
    fn ra_adapter_extracts_probe_shapes() {
        let root = temp_dir("ra_probe_shapes");
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='test'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        fs::write(
            root.join("src/lib.rs"),
            r#"
pub fn validate(value: i32) -> Result<i32, String> {
    if value < 0 {
        Err("negative".to_string())
    } else {
        Ok(value * 2)
    }
}
"#,
        )
        .unwrap();

        let adapter = RaRustSyntaxAdapter;
        let text = fs::read_to_string(root.join("src/lib.rs")).unwrap();
        let facts = adapter
            .summarize_file(&root.join("src/lib.rs"), &text)
            .unwrap();

        assert!(
            facts
                .probe_shapes
                .iter()
                .any(|p| p.kind == PROBE_SHAPE_PREDICATE),
            "Should extract predicate probe shapes"
        );
        assert!(
            facts
                .probe_shapes
                .iter()
                .any(|p| p.kind == PROBE_SHAPE_ERROR_PATH),
            "Should extract error_path probe shapes"
        );
    }

    #[test]
    fn ra_adapter_handles_parser_errors() {
        let root = temp_dir("ra_parser_error");
        let adapter = RaRustSyntaxAdapter;
        let invalid_rust = "pub fn broken( { invalid rust";
        let result = adapter.summarize_file(&root.join("invalid.rs"), invalid_rust);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("syntax errors"));
    }

    #[test]
    fn ra_adapter_changed_nodes_returns_empty_for_missing_file() {
        let _index = crate::analysis::facts::RustIndex::default();
        let adapter = RaRustSyntaxAdapter;
        let ranges = vec![TextRange {
            start_line: 1,
            start_column: 1,
            end_line: 10,
            end_column: 80,
        }];

        let nodes = adapter.changed_nodes(
            &crate::analysis::facts::FileFacts {
                path: std::path::PathBuf::from("nonexistent.rs"),
                functions: vec![],
                tests: vec![],
                calls: vec![],
                returns: vec![],
                literals: vec![],
                probe_shapes: vec![],
            },
            &ranges,
        );

        assert!(nodes.is_empty());
    }
}
