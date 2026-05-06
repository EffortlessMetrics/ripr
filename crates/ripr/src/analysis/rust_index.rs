use crate::domain::{OracleKind, OracleStrength, SymbolId};
use std::path::{Path, PathBuf};

pub const PROBE_SHAPE_PREDICATE: &str = "predicate";
pub const PROBE_SHAPE_RETURN_VALUE: &str = "return_value";
pub const PROBE_SHAPE_ERROR_PATH: &str = "error_path";
pub const PROBE_SHAPE_CALL_DELETION: &str = "call_deletion";
pub const PROBE_SHAPE_FIELD_CONSTRUCTION: &str = "field_construction";
pub const PROBE_SHAPE_SIDE_EFFECT: &str = "side_effect";
pub const PROBE_SHAPE_MATCH_ARM: &str = "match_arm";

pub use super::facts::{
    CallFact, FileFacts, FunctionFact, FunctionSummary, LiteralFact, OracleFact, ProbeShapeFact,
    ReturnFact, RustIndex, TestFact, TestSummary, build_index,
};
pub use super::syntax::{
    LexicalRustSyntaxAdapter, RaRustSyntaxAdapter, RustSyntaxAdapter, SyntaxNodeFact, TextRange,
};

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

pub fn is_test_file(path: &Path) -> bool {
    path.starts_with("tests")
        || path
            .to_string_lossy()
            .replace('\\', "/")
            .contains("/tests/")
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OracleClassification {
    pub kind: OracleKind,
    pub strength: OracleStrength,
}

pub fn classify_assertion(line: &str) -> OracleClassification {
    if is_exact_error_variant_assertion(line) {
        OracleClassification {
            kind: OracleKind::ExactErrorVariant,
            strength: OracleStrength::Strong,
        }
    } else if is_broad_error_assertion(line) {
        OracleClassification {
            kind: OracleKind::BroadError,
            strength: OracleStrength::Weak,
        }
    } else if is_exact_value_assertion(line) {
        OracleClassification {
            kind: OracleKind::ExactValue,
            strength: OracleStrength::Strong,
        }
    } else if is_snapshot_assertion(line) {
        OracleClassification {
            kind: OracleKind::Snapshot,
            strength: OracleStrength::Medium,
        }
    } else if line.contains(".unwrap(")
        || line.contains(".expect(")
        || line.contains("is_ok")
        || line.contains("is_some")
        || line.contains("is_none")
    {
        OracleClassification {
            kind: OracleKind::SmokeOnly,
            strength: OracleStrength::Smoke,
        }
    } else if line.contains("expect_") || line.contains("mock") {
        OracleClassification {
            kind: OracleKind::MockExpectation,
            strength: OracleStrength::Medium,
        }
    } else if line.contains("> 0")
        || line.contains("<")
        || line.contains(">")
        || line.contains("is_empty")
        || line.contains("contains")
        || line.contains("assert!")
    {
        OracleClassification {
            kind: OracleKind::RelationalCheck,
            strength: OracleStrength::Weak,
        }
    } else {
        OracleClassification {
            kind: OracleKind::Unknown,
            strength: OracleStrength::Unknown,
        }
    }
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

fn is_exact_error_variant_assertion(line: &str) -> bool {
    (line.contains("assert_matches!") || line.contains("matches!") || line.contains("assert_eq!"))
        && line.contains("Err(")
        && !line.contains("Err(_")
}

fn is_broad_error_assertion(line: &str) -> bool {
    line.contains("is_err") || line.contains("Err(_)")
}

fn is_exact_value_assertion(line: &str) -> bool {
    line.contains("assert_eq!")
        || line.contains("assert_ne!")
        || line.contains("assert_matches!")
        || line.contains("matches!")
}

pub fn extract_call_facts(body: &str, start_line: usize) -> Vec<CallFact> {
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

pub fn extract_return_facts(body: &str, start_line: usize) -> Vec<ReturnFact> {
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

pub fn extract_literal_facts(body: &str, start_line: usize) -> Vec<LiteralFact> {
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
        assert_eq!(file.tests[0].assertions[0].kind, OracleKind::BroadError);
        assert_eq!(file.tests[0].assertions[0].strength, OracleStrength::Weak);
    }

    #[test]
    fn classifies_exact_error_variants_separately_from_broad_error_shapes() {
        let exact = classify_assertion("assert_matches!(result, Err(AuthError::RevokedToken));");
        let broad = classify_assertion("assert_matches!(result, Err(_));");
        let ok_pattern = classify_assertion("assert_matches!(result, Ok(Value::Ready));");

        assert_eq!(exact.kind, OracleKind::ExactErrorVariant);
        assert_eq!(exact.strength, OracleStrength::Strong);
        assert_eq!(broad.kind, OracleKind::BroadError);
        assert_eq!(broad.strength, OracleStrength::Weak);
        assert_eq!(ok_pattern.kind, OracleKind::ExactValue);
        assert_eq!(ok_pattern.strength, OracleStrength::Strong);
    }

    #[test]
    fn classifies_snapshot_mock_relational_smoke_and_unknown_oracles() {
        let snapshot_cases = [
            "insta::assert_snapshot!(rendered);",
            "insta::assert_yaml_snapshot!(payload);",
            "assert_snapshot!(rendered);",
            "assert_json_snapshot!(payload);",
            "assert_debug_snapshot!(payload);",
            "assert_csv_snapshot!(payload);",
            r##"expect![[r#"ok"#]].assert_eq(&rendered);"##,
            r##"expect![[r#"ok"#]].assert_debug_eq(&rendered);"##,
            r##"expect![[r#"ok"#]].assert_json_eq(&rendered);"##,
            r#"expect_file!["snapshots/render.snap"].assert_eq(&rendered);"#,
            r#"expect_file!["snapshots/render.snap"].assert_debug_eq(&rendered);"#,
            r#"expect_file!["snapshots/render.snap"].assert_json_eq(&rendered);"#,
        ];

        for case in snapshot_cases {
            let snapshot = classify_assertion(case);
            assert_eq!(snapshot.kind, OracleKind::Snapshot, "case: {case}");
            assert_eq!(snapshot.strength, OracleStrength::Medium, "case: {case}");
        }

        let bare_expect_file = classify_assertion(r#"let expected = expect_file!["render.snap"];"#);
        let non_snapshot_method = classify_assertion("helper.assert_eq(&rendered);");
        let non_snapshot_insta_assertion = classify_assertion("insta::assert_redacted!(payload);");
        let unrelated_snapshot_macro = classify_assertion("snapshot!(rendered);");
        let mock = classify_assertion("mock.expect_publish().times(1);");
        let relational = classify_assertion("assert!(total > 0);");
        let smoke = classify_assertion("assert!(result.is_ok());");
        let unknown = classify_assertion("helper_records_observation();");

        assert_ne!(bare_expect_file.kind, OracleKind::Snapshot);
        assert_ne!(non_snapshot_method.kind, OracleKind::Snapshot);
        assert_ne!(non_snapshot_insta_assertion.kind, OracleKind::Snapshot);
        assert_ne!(unrelated_snapshot_macro.kind, OracleKind::Snapshot);
        assert_eq!(mock.kind, OracleKind::MockExpectation);
        assert_eq!(mock.strength, OracleStrength::Medium);
        assert_eq!(relational.kind, OracleKind::RelationalCheck);
        assert_eq!(relational.strength, OracleStrength::Weak);
        assert_eq!(smoke.kind, OracleKind::SmokeOnly);
        assert_eq!(smoke.strength, OracleStrength::Smoke);
        assert_eq!(unknown.kind, OracleKind::Unknown);
        assert_eq!(unknown.strength, OracleStrength::Unknown);
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
        assert_eq!(facts.tests[0].assertions[0].kind, OracleKind::ExactValue);
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
        assert_eq!(assertions[0].kind, OracleKind::SmokeOnly);
        assert_eq!(assertions[0].strength, OracleStrength::Smoke);
        assert_eq!(assertions[1].kind, OracleKind::SmokeOnly);
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
