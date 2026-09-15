use super::owners_tests::{
    collect_imports_from_statements, collect_owners_from_statements, collect_tests_from_statements,
    module_owner,
};
use super::source_utils::{line_for_range_end, line_for_range_start, text_for_range};
use super::{PythonOwner, PythonTest, expr_full_name};
use crate::domain::{LanguageId as DomainLanguageId, StaticLimitKind};
use rustpython_parser::{
    Mode,
    ast::{self, Expr, Mod, Ranged, Stmt},
    parse,
    text_size::{TextRange, TextSize},
};
use std::{
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PythonSourceFacts {
    pub(super) file: PathBuf,
    pub(super) language: &'static str,
    pub(super) owners: Vec<PythonOwner>,
    pub(super) tests: Vec<PythonTest>,
    pub(super) facts: Vec<PythonSourceFact>,
    pub(super) limitations: Vec<PythonSourceLimitation>,
    pub(super) docstring_line_ranges: Vec<RangeInclusive<usize>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PythonSourceFact {
    pub(super) kind: PythonSourceFactKind,
    pub(super) file: PathBuf,
    pub(super) owner: Option<String>,
    pub(super) start_line: usize,
    pub(super) end_line: usize,
    pub(super) start_byte: usize,
    pub(super) end_byte: usize,
    pub(super) text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum PythonSourceFactKind {
    Module,
    Class,
    Function,
    Method,
    Decorator,
    Parameter,
    Return,
    Raise,
    Predicate,
    Comparison,
    BooleanExpression,
    Call,
    Assignment,
    AttributeWrite,
    DictLiteral,
    ListLiteral,
    SetLiteral,
    StringLiteral,
    PrintCall,
    LogCall,
}

impl PythonSourceFactKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Class => "class",
            Self::Function => "function",
            Self::Method => "method",
            Self::Decorator => "decorator",
            Self::Parameter => "parameter",
            Self::Return => "return",
            Self::Raise => "raise",
            Self::Predicate => "predicate",
            Self::Comparison => "comparison",
            Self::BooleanExpression => "boolean_expression",
            Self::Call => "call",
            Self::Assignment => "assignment",
            Self::AttributeWrite => "attribute_write",
            Self::DictLiteral => "dict_literal",
            Self::ListLiteral => "list_literal",
            Self::SetLiteral => "set_literal",
            Self::StringLiteral => "string_literal",
            Self::PrintCall => "print_call",
            Self::LogCall => "log_call",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PythonSourceLimitation {
    pub(super) kind: StaticLimitKind,
    pub(super) evidence: String,
    pub(super) missing: String,
}

/// Detect the Python test framework for a workspace root (#2106), using
/// the marker set the adapter's code-level detection implies:
///
/// - pytest: `pytest.ini`, `conftest.py`, or a pytest section in
///   `pyproject.toml` / `setup.cfg` / `tox.ini` (a bare pyproject.toml is
///   PEP 517 packaging, not pytest evidence — #2183 review);
/// - unittest: no config exists by design, so detection uses bounded code
///   evidence (`import unittest` in a `test_*.py` file at the root or in
///   `tests/` / `test/`), matching what the adapter detects from source.
///
/// Fail-closed: `None` when no marker matches — callers must report
/// "not detected", never guess.
pub(crate) fn detect_python_test_framework(root: &Path) -> Option<&'static str> {
    if root.join("pytest.ini").exists()
        || root.join("conftest.py").exists()
        // A bare pyproject.toml is PEP 517 packaging, not pytest evidence
        // (#2183 review); only an actual pytest section counts.
        || ini_section_present(&root.join("pyproject.toml"), "[tool.pytest.ini_options]")
        || ini_section_present(&root.join("setup.cfg"), "[tool:pytest]")
        || ini_section_present(&root.join("tox.ini"), "[pytest]")
    {
        return Some("pytest");
    }
    for dir in [root.to_path_buf(), root.join("tests"), root.join("test")] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten().take(64) {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if !name.starts_with("test_") || !name.ends_with(".py") {
                continue;
            }
            // Bounded evidence read: the import line is at the top of the
            // file, so a small prefix suffices.
            let Ok(bytes) = std::fs::read(entry.path()) else {
                continue;
            };
            let prefix = &bytes[..bytes.len().min(4096)];
            if String::from_utf8_lossy(prefix)
                .lines()
                .any(is_unittest_import_line)
            {
                return Some("unittest");
            }
        }
    }
    None
}

/// Whether a line is a real unittest import statement — `import unittest`,
/// `import unittest as ...`, or `from unittest import ...` — at a token
/// boundary. Comment lines and lookalike identifiers (`import unittesting`)
/// do not count (#2106 review).
fn is_unittest_import_line(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.starts_with('#') {
        return false;
    }
    if let Some(rest) = trimmed.strip_prefix("from ") {
        return rest
            .strip_prefix("unittest")
            .is_some_and(|rest| rest.starts_with(" import"));
    }
    trimmed
        .strip_prefix("import ")
        .is_some_and(|rest| rest == "unittest" || rest.starts_with("unittest "))
}

/// Whether an INI-style file exists and contains the given section header.
fn ini_section_present(path: &Path, section: &str) -> bool {
    std::fs::read_to_string(path)
        .map(|text| text.lines().any(|line| line.trim() == section))
        .unwrap_or(false)
}

pub(super) fn parse_module_result(path: &Path, source: &str) -> Result<Mod, String> {
    let source_path = path.to_string_lossy();
    let module = parse(source, Mode::Module, source_path.as_ref())
        .map_err(|err| format!("parse_error: {err}"))?;
    match module {
        Mod::Module(_) => Ok(module),
        _ => Err("parse_error: expected Python module".to_string()),
    }
}

#[cfg(test)]
pub(super) fn parse_module(path: &Path, source: &str) -> Option<Mod> {
    parse_module_result(path, source).ok()
}

/// The producer-owned parse-error row of a failed snapshot, if any.
///
/// `extract_source_facts` types a failed module parse as an
/// `UnsupportedSyntax` limitation whose evidence carries the
/// `source_fact_parse_error` prefix; this keeps the failure-shape knowledge
/// inside the producer that owns it. The repo-mode evidence producer
/// (#3554 PR B) uses this to emit a typed `ParseFailure` limitation and drop
/// the file from the analyzed denominator.
pub(super) fn source_facts_parse_error(
    facts: &PythonSourceFacts,
) -> Option<&PythonSourceLimitation> {
    facts.limitations.iter().find(|limitation| {
        limitation.kind == StaticLimitKind::UnsupportedSyntax
            && limitation.evidence.starts_with("source_fact_parse_error")
    })
}

pub(super) fn extract_source_facts(file: &Path, source: &str) -> PythonSourceFacts {
    let mut snapshot = PythonSourceFacts {
        file: file.to_path_buf(),
        language: DomainLanguageId::Python.as_str(),
        owners: Vec::new(),
        tests: Vec::new(),
        facts: Vec::new(),
        limitations: Vec::new(),
        docstring_line_ranges: Vec::new(),
    };
    let module = match parse_module_result(file, source) {
        Ok(Mod::Module(module)) => module,
        Ok(_) => {
            snapshot.limitations.push(PythonSourceLimitation {
                kind: StaticLimitKind::UnsupportedSyntax,
                evidence: "source_fact_parse_error: parse_error: expected Python module"
                    .to_string(),
                missing: "Static limit `unsupported_syntax`: malformed Python prevented source-fact extraction.".to_string(),
            });
            return snapshot;
        }
        Err(parse_reason) => {
            snapshot.limitations.push(PythonSourceLimitation {
                kind: StaticLimitKind::UnsupportedSyntax,
                evidence: format!("source_fact_parse_error: {parse_reason}"),
                missing: "Static limit `unsupported_syntax`: malformed Python prevented source-fact extraction.".to_string(),
            });
            return snapshot;
        }
    };

    let module_range = TextRange::new(
        TextSize::from(0),
        TextSize::from(u32::try_from(source.len()).unwrap_or(u32::MAX)),
    );
    push_source_fact(
        &mut snapshot.facts,
        file,
        source,
        PythonSourceFactKind::Module,
        None,
        module_range,
    );

    let imports = collect_imports_from_statements(file, &module.body);
    collect_owners_from_statements(
        file,
        source,
        &module.body,
        None,
        &imports,
        &mut snapshot.owners,
    );
    snapshot
        .owners
        .push(module_owner(file, source, module_range, &imports));
    collect_tests_from_statements(
        file,
        source,
        &module.body,
        None,
        false,
        &imports,
        &mut snapshot.tests,
    );
    collect_source_facts_from_statements(
        file,
        source,
        &module.body,
        None,
        None,
        &mut snapshot.facts,
    );
    collect_docstring_line_ranges(source, &module.body, &mut snapshot.docstring_line_ranges);
    snapshot
}

/// Collects the line spans of real Python docstrings from parsed scope bodies.
///
/// A docstring is the first statement of a module, function, async function, or
/// class body when that statement is a plain string constant. Assigned strings,
/// f-strings, and string expressions inside control-flow blocks are deliberately
/// excluded: they are not docstrings, even when they use triple quotes.
fn collect_docstring_line_ranges(
    source: &str,
    scope_body: &[Stmt],
    out: &mut Vec<RangeInclusive<usize>>,
) {
    if let Some(Stmt::Expr(expr_stmt)) = scope_body.first()
        && matches!(
            expr_stmt.value.as_ref(),
            Expr::Constant(constant) if matches!(&constant.value, ast::Constant::Str(_))
        )
    {
        push_docstring_only_line_ranges(source, expr_stmt.value.range(), out);
    }
    collect_nested_docstring_scopes(source, scope_body, out);
}

/// Adds only physical lines whose non-whitespace content is wholly inside the
/// docstring token. A docstring may share a line with behavioral code through a
/// semicolon; such boundary lines must remain analyzable.
fn push_docstring_only_line_ranges(
    source: &str,
    range: TextRange,
    out: &mut Vec<RangeInclusive<usize>>,
) {
    let start_offset = usize::from(range.start());
    let end_offset = usize::from(range.end());
    let start_line = line_for_range_start(source, range);
    let end_line = line_for_range_end(source, range);
    let start_line_offset = source[..start_offset]
        .rfind('\n')
        .map_or(0, |offset| offset + 1);
    let end_line_offset = source[end_offset..]
        .find('\n')
        .map_or(source.len(), |offset| end_offset + offset);
    let prefix_is_whitespace = source[start_line_offset..start_offset].trim().is_empty();
    let suffix_is_whitespace = source[end_offset..end_line_offset].trim().is_empty();

    if start_line == end_line {
        if prefix_is_whitespace && suffix_is_whitespace {
            out.push(start_line..=end_line);
        }
        return;
    }

    let first_safe_line = if prefix_is_whitespace {
        start_line
    } else {
        start_line + 1
    };
    let last_safe_line = if suffix_is_whitespace {
        end_line
    } else {
        end_line.saturating_sub(1)
    };
    if first_safe_line <= last_safe_line {
        out.push(first_safe_line..=last_safe_line);
    }
}

fn collect_nested_docstring_scopes(
    source: &str,
    statements: &[Stmt],
    out: &mut Vec<RangeInclusive<usize>>,
) {
    for statement in statements {
        match statement {
            Stmt::FunctionDef(function) => {
                collect_docstring_line_ranges(source, &function.body, out);
            }
            Stmt::AsyncFunctionDef(function) => {
                collect_docstring_line_ranges(source, &function.body, out);
            }
            Stmt::ClassDef(class) => {
                collect_docstring_line_ranges(source, &class.body, out);
            }
            Stmt::If(statement) => {
                collect_nested_docstring_scopes(source, &statement.body, out);
                collect_nested_docstring_scopes(source, &statement.orelse, out);
            }
            Stmt::For(statement) => {
                collect_nested_docstring_scopes(source, &statement.body, out);
                collect_nested_docstring_scopes(source, &statement.orelse, out);
            }
            Stmt::AsyncFor(statement) => {
                collect_nested_docstring_scopes(source, &statement.body, out);
                collect_nested_docstring_scopes(source, &statement.orelse, out);
            }
            Stmt::While(statement) => {
                collect_nested_docstring_scopes(source, &statement.body, out);
                collect_nested_docstring_scopes(source, &statement.orelse, out);
            }
            Stmt::With(statement) => {
                collect_nested_docstring_scopes(source, &statement.body, out);
            }
            Stmt::AsyncWith(statement) => {
                collect_nested_docstring_scopes(source, &statement.body, out);
            }
            Stmt::Try(statement) => {
                collect_nested_docstring_scopes(source, &statement.body, out);
                for handler in &statement.handlers {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    collect_nested_docstring_scopes(source, &handler.body, out);
                }
                collect_nested_docstring_scopes(source, &statement.orelse, out);
                collect_nested_docstring_scopes(source, &statement.finalbody, out);
            }
            Stmt::TryStar(statement) => {
                collect_nested_docstring_scopes(source, &statement.body, out);
                for handler in &statement.handlers {
                    let ast::ExceptHandler::ExceptHandler(handler) = handler;
                    collect_nested_docstring_scopes(source, &handler.body, out);
                }
                collect_nested_docstring_scopes(source, &statement.orelse, out);
                collect_nested_docstring_scopes(source, &statement.finalbody, out);
            }
            Stmt::Match(statement) => {
                for case in &statement.cases {
                    collect_nested_docstring_scopes(source, &case.body, out);
                }
            }
            _ => {}
        }
    }
}

pub(super) fn source_fact_snapshot_observation(facts: &PythonSourceFacts) -> usize {
    let mut score = facts.file.components().count() + facts.language.len();
    score = score.saturating_add(facts.owners.len());
    score = score.saturating_add(facts.tests.len());
    score = score.saturating_add(facts.docstring_line_ranges.len());
    for fact in &facts.facts {
        score = score.saturating_add(fact.kind.as_str().len());
        score = score.saturating_add(fact.file.components().count());
        score = score.saturating_add(fact.owner.as_deref().unwrap_or_default().len());
        score = score.saturating_add(fact.start_line);
        score = score.saturating_add(fact.end_line);
        score = score.saturating_add(fact.start_byte);
        score = score.saturating_add(fact.end_byte);
        score = score.saturating_add(fact.text.len());
    }
    for limitation in &facts.limitations {
        score = score.saturating_add(limitation.kind.as_str().len());
        score = score.saturating_add(limitation.evidence.len());
        score = score.saturating_add(limitation.missing.len());
    }
    score
}

fn push_source_fact(
    out: &mut Vec<PythonSourceFact>,
    file: &Path,
    source: &str,
    kind: PythonSourceFactKind,
    owner: Option<&str>,
    range: TextRange,
) {
    out.push(PythonSourceFact {
        kind,
        file: file.to_path_buf(),
        owner: owner.map(str::to_string),
        start_line: line_for_range_start(source, range),
        end_line: line_for_range_end(source, range),
        start_byte: usize::from(range.start()),
        end_byte: usize::from(range.end()),
        text: text_for_range(source, range).trim().to_string(),
    });
}

fn collect_source_facts_from_statements(
    file: &Path,
    source: &str,
    statements: &[Stmt],
    class_context: Option<&str>,
    current_owner: Option<&str>,
    out: &mut Vec<PythonSourceFact>,
) {
    for stmt in statements {
        match stmt {
            Stmt::FunctionDef(function) => {
                collect_source_facts_from_function(
                    PythonFunctionSourceContext {
                        file,
                        source,
                        class_context,
                        name: function.name.as_str(),
                        range: function.range,
                        args: &function.args,
                        decorators: &function.decorator_list,
                        body: &function.body,
                    },
                    out,
                );
            }
            Stmt::AsyncFunctionDef(function) => {
                collect_source_facts_from_function(
                    PythonFunctionSourceContext {
                        file,
                        source,
                        class_context,
                        name: function.name.as_str(),
                        range: function.range,
                        args: &function.args,
                        decorators: &function.decorator_list,
                        body: &function.body,
                    },
                    out,
                );
            }
            Stmt::ClassDef(class) => {
                let owner = current_owner.unwrap_or(class.name.as_str());
                push_source_fact(
                    out,
                    file,
                    source,
                    PythonSourceFactKind::Class,
                    Some(owner),
                    class.range,
                );
                for decorator in &class.decorator_list {
                    collect_decorator_fact(file, source, decorator, Some(owner), out);
                }
                collect_source_facts_from_statements(
                    file,
                    source,
                    &class.body,
                    Some(class.name.as_str()),
                    Some(class.name.as_str()),
                    out,
                );
            }
            Stmt::Return(return_stmt) => {
                push_source_fact(
                    out,
                    file,
                    source,
                    PythonSourceFactKind::Return,
                    current_owner,
                    return_stmt.range,
                );
                if let Some(value) = &return_stmt.value {
                    collect_source_facts_from_expr(file, source, value, current_owner, out);
                }
            }
            Stmt::Raise(raise_stmt) => {
                push_source_fact(
                    out,
                    file,
                    source,
                    PythonSourceFactKind::Raise,
                    current_owner,
                    raise_stmt.range,
                );
                if let Some(exc) = &raise_stmt.exc {
                    collect_source_facts_from_expr(file, source, exc, current_owner, out);
                }
                if let Some(cause) = &raise_stmt.cause {
                    collect_source_facts_from_expr(file, source, cause, current_owner, out);
                }
            }
            Stmt::If(if_stmt) => {
                push_source_fact(
                    out,
                    file,
                    source,
                    PythonSourceFactKind::Predicate,
                    current_owner,
                    if_stmt.test.range(),
                );
                collect_source_facts_from_expr(file, source, &if_stmt.test, current_owner, out);
                collect_source_facts_from_statements(
                    file,
                    source,
                    &if_stmt.body,
                    class_context,
                    current_owner,
                    out,
                );
                collect_source_facts_from_statements(
                    file,
                    source,
                    &if_stmt.orelse,
                    class_context,
                    current_owner,
                    out,
                );
            }
            Stmt::While(while_stmt) => {
                push_source_fact(
                    out,
                    file,
                    source,
                    PythonSourceFactKind::Predicate,
                    current_owner,
                    while_stmt.test.range(),
                );
                collect_source_facts_from_expr(file, source, &while_stmt.test, current_owner, out);
                collect_source_facts_from_statements(
                    file,
                    source,
                    &while_stmt.body,
                    class_context,
                    current_owner,
                    out,
                );
                collect_source_facts_from_statements(
                    file,
                    source,
                    &while_stmt.orelse,
                    class_context,
                    current_owner,
                    out,
                );
            }
            Stmt::For(for_stmt) => {
                collect_source_facts_from_expr(file, source, &for_stmt.target, current_owner, out);
                collect_source_facts_from_expr(file, source, &for_stmt.iter, current_owner, out);
                collect_source_facts_from_statements(
                    file,
                    source,
                    &for_stmt.body,
                    class_context,
                    current_owner,
                    out,
                );
                collect_source_facts_from_statements(
                    file,
                    source,
                    &for_stmt.orelse,
                    class_context,
                    current_owner,
                    out,
                );
            }
            Stmt::AsyncFor(for_stmt) => {
                collect_source_facts_from_expr(file, source, &for_stmt.target, current_owner, out);
                collect_source_facts_from_expr(file, source, &for_stmt.iter, current_owner, out);
                collect_source_facts_from_statements(
                    file,
                    source,
                    &for_stmt.body,
                    class_context,
                    current_owner,
                    out,
                );
                collect_source_facts_from_statements(
                    file,
                    source,
                    &for_stmt.orelse,
                    class_context,
                    current_owner,
                    out,
                );
            }
            Stmt::Match(match_stmt) => {
                push_source_fact(
                    out,
                    file,
                    source,
                    PythonSourceFactKind::Predicate,
                    current_owner,
                    match_stmt.subject.range(),
                );
                collect_source_facts_from_expr(
                    file,
                    source,
                    &match_stmt.subject,
                    current_owner,
                    out,
                );
                for case in &match_stmt.cases {
                    if let Some(guard) = &case.guard {
                        collect_source_facts_from_expr(file, source, guard, current_owner, out);
                    }
                    collect_source_facts_from_statements(
                        file,
                        source,
                        &case.body,
                        class_context,
                        current_owner,
                        out,
                    );
                }
            }
            Stmt::Assign(assign) => {
                push_source_fact(
                    out,
                    file,
                    source,
                    PythonSourceFactKind::Assignment,
                    current_owner,
                    assign.range,
                );
                for target in &assign.targets {
                    collect_assignment_target_facts(file, source, target, current_owner, out);
                }
                collect_source_facts_from_expr(file, source, &assign.value, current_owner, out);
            }
            Stmt::AnnAssign(assign) => {
                push_source_fact(
                    out,
                    file,
                    source,
                    PythonSourceFactKind::Assignment,
                    current_owner,
                    assign.range,
                );
                collect_assignment_target_facts(file, source, &assign.target, current_owner, out);
                collect_source_facts_from_expr(
                    file,
                    source,
                    &assign.annotation,
                    current_owner,
                    out,
                );
                if let Some(value) = &assign.value {
                    collect_source_facts_from_expr(file, source, value, current_owner, out);
                }
            }
            Stmt::AugAssign(assign) => {
                push_source_fact(
                    out,
                    file,
                    source,
                    PythonSourceFactKind::Assignment,
                    current_owner,
                    assign.range,
                );
                collect_assignment_target_facts(file, source, &assign.target, current_owner, out);
                collect_source_facts_from_expr(file, source, &assign.value, current_owner, out);
            }
            Stmt::Expr(expr_stmt) => {
                collect_source_facts_from_expr(file, source, &expr_stmt.value, current_owner, out);
            }
            Stmt::With(with_stmt) => {
                for item in &with_stmt.items {
                    collect_source_facts_from_expr(
                        file,
                        source,
                        &item.context_expr,
                        current_owner,
                        out,
                    );
                    if let Some(optional_vars) = &item.optional_vars {
                        collect_assignment_target_facts(
                            file,
                            source,
                            optional_vars,
                            current_owner,
                            out,
                        );
                    }
                }
                collect_source_facts_from_statements(
                    file,
                    source,
                    &with_stmt.body,
                    class_context,
                    current_owner,
                    out,
                );
            }
            Stmt::AsyncWith(with_stmt) => {
                for item in &with_stmt.items {
                    collect_source_facts_from_expr(
                        file,
                        source,
                        &item.context_expr,
                        current_owner,
                        out,
                    );
                    if let Some(optional_vars) = &item.optional_vars {
                        collect_assignment_target_facts(
                            file,
                            source,
                            optional_vars,
                            current_owner,
                            out,
                        );
                    }
                }
                collect_source_facts_from_statements(
                    file,
                    source,
                    &with_stmt.body,
                    class_context,
                    current_owner,
                    out,
                );
            }
            Stmt::Try(try_stmt) => {
                collect_source_facts_from_statements(
                    file,
                    source,
                    &try_stmt.body,
                    class_context,
                    current_owner,
                    out,
                );
                collect_source_facts_from_except_handlers(
                    file,
                    source,
                    &try_stmt.handlers,
                    class_context,
                    current_owner,
                    out,
                );
                collect_source_facts_from_statements(
                    file,
                    source,
                    &try_stmt.orelse,
                    class_context,
                    current_owner,
                    out,
                );
                collect_source_facts_from_statements(
                    file,
                    source,
                    &try_stmt.finalbody,
                    class_context,
                    current_owner,
                    out,
                );
            }
            Stmt::TryStar(try_stmt) => {
                collect_source_facts_from_statements(
                    file,
                    source,
                    &try_stmt.body,
                    class_context,
                    current_owner,
                    out,
                );
                collect_source_facts_from_except_handlers(
                    file,
                    source,
                    &try_stmt.handlers,
                    class_context,
                    current_owner,
                    out,
                );
                collect_source_facts_from_statements(
                    file,
                    source,
                    &try_stmt.orelse,
                    class_context,
                    current_owner,
                    out,
                );
                collect_source_facts_from_statements(
                    file,
                    source,
                    &try_stmt.finalbody,
                    class_context,
                    current_owner,
                    out,
                );
            }
            _ => {}
        }
    }
}

struct PythonFunctionSourceContext<'a> {
    file: &'a Path,
    source: &'a str,
    class_context: Option<&'a str>,
    name: &'a str,
    range: TextRange,
    args: &'a ast::Arguments,
    decorators: &'a [Expr],
    body: &'a [Stmt],
}

fn collect_source_facts_from_function(
    context: PythonFunctionSourceContext<'_>,
    out: &mut Vec<PythonSourceFact>,
) {
    let qualified_name = context
        .class_context
        .map(|class| format!("{class}.{}", context.name))
        .unwrap_or_else(|| context.name.to_string());
    let kind = if context.class_context.is_some() {
        PythonSourceFactKind::Method
    } else {
        PythonSourceFactKind::Function
    };
    push_source_fact(
        out,
        context.file,
        context.source,
        kind,
        Some(&qualified_name),
        context.range,
    );
    for decorator in context.decorators {
        collect_decorator_fact(
            context.file,
            context.source,
            decorator,
            Some(&qualified_name),
            out,
        );
    }
    collect_parameter_facts(
        context.file,
        context.source,
        context.args,
        Some(&qualified_name),
        out,
    );
    collect_source_facts_from_statements(
        context.file,
        context.source,
        context.body,
        context.class_context,
        Some(&qualified_name),
        out,
    );
}

fn collect_decorator_fact(
    file: &Path,
    source: &str,
    decorator: &Expr,
    owner: Option<&str>,
    out: &mut Vec<PythonSourceFact>,
) {
    push_source_fact(
        out,
        file,
        source,
        PythonSourceFactKind::Decorator,
        owner,
        decorator.range(),
    );
    collect_source_facts_from_expr(file, source, decorator, owner, out);
}

fn collect_parameter_facts(
    file: &Path,
    source: &str,
    args: &ast::Arguments,
    owner: Option<&str>,
    out: &mut Vec<PythonSourceFact>,
) {
    for arg in args
        .posonlyargs
        .iter()
        .chain(args.args.iter())
        .chain(args.kwonlyargs.iter())
    {
        push_source_fact(
            out,
            file,
            source,
            PythonSourceFactKind::Parameter,
            owner,
            arg.def.range,
        );
        if let Some(annotation) = &arg.def.annotation {
            collect_source_facts_from_expr(file, source, annotation, owner, out);
        }
        if let Some(default) = &arg.default {
            collect_source_facts_from_expr(file, source, default, owner, out);
        }
    }
    if let Some(arg) = &args.vararg {
        push_source_fact(
            out,
            file,
            source,
            PythonSourceFactKind::Parameter,
            owner,
            arg.range,
        );
    }
    if let Some(arg) = &args.kwarg {
        push_source_fact(
            out,
            file,
            source,
            PythonSourceFactKind::Parameter,
            owner,
            arg.range,
        );
    }
}

fn collect_assignment_target_facts(
    file: &Path,
    source: &str,
    target: &Expr,
    owner: Option<&str>,
    out: &mut Vec<PythonSourceFact>,
) {
    if let Expr::Attribute(attribute) = target {
        push_source_fact(
            out,
            file,
            source,
            PythonSourceFactKind::AttributeWrite,
            owner,
            attribute.range,
        );
    }
    collect_source_facts_from_expr(file, source, target, owner, out);
}

fn collect_source_facts_from_except_handlers(
    file: &Path,
    source: &str,
    handlers: &[ast::ExceptHandler],
    class_context: Option<&str>,
    current_owner: Option<&str>,
    out: &mut Vec<PythonSourceFact>,
) {
    for handler in handlers {
        let ast::ExceptHandler::ExceptHandler(handler) = handler;
        if let Some(type_expr) = &handler.type_ {
            collect_source_facts_from_expr(file, source, type_expr, current_owner, out);
        }
        collect_source_facts_from_statements(
            file,
            source,
            &handler.body,
            class_context,
            current_owner,
            out,
        );
    }
}

fn collect_source_facts_from_expr(
    file: &Path,
    source: &str,
    expr: &Expr,
    owner: Option<&str>,
    out: &mut Vec<PythonSourceFact>,
) {
    match expr {
        Expr::BoolOp(bool_op) => {
            push_source_fact(
                out,
                file,
                source,
                PythonSourceFactKind::BooleanExpression,
                owner,
                bool_op.range,
            );
            for value in &bool_op.values {
                collect_source_facts_from_expr(file, source, value, owner, out);
            }
        }
        Expr::NamedExpr(named) => {
            collect_assignment_target_facts(file, source, &named.target, owner, out);
            collect_source_facts_from_expr(file, source, &named.value, owner, out);
        }
        Expr::BinOp(bin_op) => {
            collect_source_facts_from_expr(file, source, &bin_op.left, owner, out);
            collect_source_facts_from_expr(file, source, &bin_op.right, owner, out);
        }
        Expr::UnaryOp(unary) => {
            collect_source_facts_from_expr(file, source, &unary.operand, owner, out);
        }
        Expr::Lambda(lambda) => {
            collect_parameter_facts(file, source, &lambda.args, owner, out);
            collect_source_facts_from_expr(file, source, &lambda.body, owner, out);
        }
        Expr::IfExp(if_exp) => {
            push_source_fact(
                out,
                file,
                source,
                PythonSourceFactKind::Predicate,
                owner,
                if_exp.test.range(),
            );
            collect_source_facts_from_expr(file, source, &if_exp.test, owner, out);
            collect_source_facts_from_expr(file, source, &if_exp.body, owner, out);
            collect_source_facts_from_expr(file, source, &if_exp.orelse, owner, out);
        }
        Expr::Dict(dict) => {
            push_source_fact(
                out,
                file,
                source,
                PythonSourceFactKind::DictLiteral,
                owner,
                dict.range,
            );
            for key in dict.keys.iter().flatten() {
                collect_source_facts_from_expr(file, source, key, owner, out);
            }
            for value in &dict.values {
                collect_source_facts_from_expr(file, source, value, owner, out);
            }
        }
        Expr::Set(set) => {
            push_source_fact(
                out,
                file,
                source,
                PythonSourceFactKind::SetLiteral,
                owner,
                set.range,
            );
            for value in &set.elts {
                collect_source_facts_from_expr(file, source, value, owner, out);
            }
        }
        Expr::ListComp(list_comp) => {
            collect_source_facts_from_expr(file, source, &list_comp.elt, owner, out);
            collect_source_facts_from_comprehensions(
                file,
                source,
                &list_comp.generators,
                owner,
                out,
            );
        }
        Expr::SetComp(set_comp) => {
            collect_source_facts_from_expr(file, source, &set_comp.elt, owner, out);
            collect_source_facts_from_comprehensions(
                file,
                source,
                &set_comp.generators,
                owner,
                out,
            );
        }
        Expr::DictComp(dict_comp) => {
            collect_source_facts_from_expr(file, source, &dict_comp.key, owner, out);
            collect_source_facts_from_expr(file, source, &dict_comp.value, owner, out);
            collect_source_facts_from_comprehensions(
                file,
                source,
                &dict_comp.generators,
                owner,
                out,
            );
        }
        Expr::GeneratorExp(generator) => {
            collect_source_facts_from_expr(file, source, &generator.elt, owner, out);
            collect_source_facts_from_comprehensions(
                file,
                source,
                &generator.generators,
                owner,
                out,
            );
        }
        Expr::Await(await_expr) => {
            collect_source_facts_from_expr(file, source, &await_expr.value, owner, out);
        }
        Expr::Yield(yield_expr) => {
            if let Some(value) = &yield_expr.value {
                collect_source_facts_from_expr(file, source, value, owner, out);
            }
        }
        Expr::YieldFrom(yield_expr) => {
            collect_source_facts_from_expr(file, source, &yield_expr.value, owner, out);
        }
        Expr::Compare(compare) => {
            push_source_fact(
                out,
                file,
                source,
                PythonSourceFactKind::Comparison,
                owner,
                compare.range,
            );
            collect_source_facts_from_expr(file, source, &compare.left, owner, out);
            for comparator in &compare.comparators {
                collect_source_facts_from_expr(file, source, comparator, owner, out);
            }
        }
        Expr::Call(call) => {
            let call_name = expr_full_name(call.func.as_ref());
            if call_name.as_deref() == Some("print") {
                push_source_fact(
                    out,
                    file,
                    source,
                    PythonSourceFactKind::PrintCall,
                    owner,
                    call.range,
                );
            }
            if call_name.as_deref().is_some_and(is_log_call_name) {
                push_source_fact(
                    out,
                    file,
                    source,
                    PythonSourceFactKind::LogCall,
                    owner,
                    call.range,
                );
            }
            push_source_fact(
                out,
                file,
                source,
                PythonSourceFactKind::Call,
                owner,
                call.range,
            );
            collect_source_facts_from_expr(file, source, &call.func, owner, out);
            for arg in &call.args {
                collect_source_facts_from_expr(file, source, arg, owner, out);
            }
            for keyword in &call.keywords {
                collect_source_facts_from_expr(file, source, &keyword.value, owner, out);
            }
        }
        Expr::FormattedValue(formatted) => {
            collect_source_facts_from_expr(file, source, &formatted.value, owner, out);
            if let Some(format_spec) = &formatted.format_spec {
                collect_source_facts_from_expr(file, source, format_spec, owner, out);
            }
        }
        Expr::JoinedStr(joined) => {
            push_source_fact(
                out,
                file,
                source,
                PythonSourceFactKind::StringLiteral,
                owner,
                joined.range,
            );
            for value in &joined.values {
                collect_source_facts_from_expr(file, source, value, owner, out);
            }
        }
        Expr::Constant(constant) => {
            if matches!(&constant.value, ast::Constant::Str(_)) {
                push_source_fact(
                    out,
                    file,
                    source,
                    PythonSourceFactKind::StringLiteral,
                    owner,
                    constant.range,
                );
            }
        }
        Expr::Attribute(attribute) => {
            collect_source_facts_from_expr(file, source, &attribute.value, owner, out);
        }
        Expr::Subscript(subscript) => {
            collect_source_facts_from_expr(file, source, &subscript.value, owner, out);
            collect_source_facts_from_expr(file, source, &subscript.slice, owner, out);
        }
        Expr::Starred(starred) => {
            collect_source_facts_from_expr(file, source, &starred.value, owner, out);
        }
        Expr::List(list) => {
            push_source_fact(
                out,
                file,
                source,
                PythonSourceFactKind::ListLiteral,
                owner,
                list.range,
            );
            for value in &list.elts {
                collect_source_facts_from_expr(file, source, value, owner, out);
            }
        }
        Expr::Tuple(tuple) => {
            for value in &tuple.elts {
                collect_source_facts_from_expr(file, source, value, owner, out);
            }
        }
        Expr::Slice(slice) => {
            if let Some(lower) = &slice.lower {
                collect_source_facts_from_expr(file, source, lower, owner, out);
            }
            if let Some(upper) = &slice.upper {
                collect_source_facts_from_expr(file, source, upper, owner, out);
            }
            if let Some(step) = &slice.step {
                collect_source_facts_from_expr(file, source, step, owner, out);
            }
        }
        Expr::Name(_) => {}
    }
}

fn collect_source_facts_from_comprehensions(
    file: &Path,
    source: &str,
    comprehensions: &[ast::Comprehension],
    owner: Option<&str>,
    out: &mut Vec<PythonSourceFact>,
) {
    for comprehension in comprehensions {
        collect_assignment_target_facts(file, source, &comprehension.target, owner, out);
        collect_source_facts_from_expr(file, source, &comprehension.iter, owner, out);
        for guard in &comprehension.ifs {
            push_source_fact(
                out,
                file,
                source,
                PythonSourceFactKind::Predicate,
                owner,
                guard.range(),
            );
            collect_source_facts_from_expr(file, source, guard, owner, out);
        }
    }
}

fn is_log_call_name(name: &str) -> bool {
    matches!(
        name.rsplit('.').next(),
        Some("debug" | "info" | "warning" | "warn" | "error" | "exception" | "critical" | "log")
    ) && (name.starts_with("logging.") || name.starts_with("logger."))
}
