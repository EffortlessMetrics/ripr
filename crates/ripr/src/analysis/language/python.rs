//! Python preview adapter.
//!
//! See `docs/specs/RIPR-SPEC-0028-python-preview-static-facts.md` and
//! `docs/adr/0009-python-parser-substrate.md`.
//!
//! This slice extracts the first useful syntax-first Python facts:
//!
//! - owners for module functions, async functions, class methods, and
//!   `@staticmethod` / `@classmethod` methods;
//! - pytest `test_*` functions, parametrized pytest tests, and
//!   `unittest.TestCase.test_*` methods;
//! - pytest, unittest, and mock assertion/oracle facts;
//! - related-test references by direct calls, import-alias calls, and
//!   conservative same-stem proximity.
//!
//! Import-graph matching, static limits, editor routing, generated tests,
//! runtime execution, and provider calls remain out of scope.
//! Strong exact-value assertions can produce `exposed`; weaker or unknown
//! related-test oracles produce `weakly_exposed`; missing related tests produce
//! `no_static_path`.

use super::super::{AnalysisOptions, diff::ChangedFile};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::config::OraclePolicy;
use crate::domain::{
    Confidence, DeltaKind, ExposureClass, Finding, FindingCanonicalGap, FlowSinkFact, FlowSinkKind,
    LanguageId as DomainLanguageId, LanguageStatus, MissingDiscriminatorFact, OracleKind,
    OracleStrength, OwnerKind, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence,
    RiprEvidence, SourceLocation, StageEvidence, StageState, StaticLimitKind, StopReason, SymbolId,
};
use rustpython_parser::{
    Mode,
    ast::{self, Expr, Mod, Ranged, Stmt},
    parse,
    text_size::{TextRange, TextSize},
};
use std::path::{Path, PathBuf};
mod source_utils;
#[cfg(test)]
use source_utils::line_for_offset;
use source_utils::{
    is_test_file, line_for_range_end, line_for_range_start, normalized_path, text_for_range,
};

const PYTHON_WORKSPACE_EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    ".ripr",
    ".direnv",
    "__pycache__",
    ".venv",
    "venv",
    "env",
    ".tox",
    ".nox",
    "site-packages",
    ".pytest_cache",
    ".mypy_cache",
    "dist",
    "build",
];

/// Python preview adapter.
///
/// Stateless: routing, parsing, and per-file extraction only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PythonAdapter;

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonOwner {
    name: String,
    qualified_name: String,
    file: PathBuf,
    start_line: usize,
    end_line: usize,
    owner_kind: Option<OwnerKind>,
    decorators: Vec<String>,
    imports: Vec<PythonImport>,
}

impl PythonOwner {
    fn symbol_id(&self) -> SymbolId {
        SymbolId(format!(
            "python:{}::{}",
            normalized_path(&self.file),
            self.qualified_name
        ))
    }

    fn is_module_owner(&self) -> bool {
        self.qualified_name == "<module>"
    }

    fn specificity_rank(&self) -> usize {
        if self.owner_kind.is_some() {
            0
        } else if self.is_module_owner() {
            2
        } else {
            1
        }
    }

    fn span_width(&self) -> usize {
        self.end_line.saturating_sub(self.start_line)
    }

    fn kind_label(&self) -> &'static str {
        match self.owner_kind {
            Some(kind) => kind.as_str(),
            None if self.is_module_owner() => "module_function",
            None => "class",
        }
    }

    fn missing_test_reference(&self) -> String {
        if self.is_module_owner() {
            format!("module-level behavior in `{}`", normalized_path(&self.file))
        } else {
            format!("`{}(`", self.name)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonTest {
    name: String,
    qualified_name: String,
    file: PathBuf,
    line: usize,
    body_text: String,
    imports: Vec<PythonImport>,
    decorators: Vec<String>,
    fixtures: Vec<String>,
    parametrized: bool,
    framework: &'static str,
    assertions: Vec<PythonAssertion>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonImport {
    imported: String,
    alias: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonAssertion {
    text: String,
    line: usize,
    oracle_kind: OracleKind,
    oracle_strength: OracleStrength,
    oracle_shape: PythonOracleShape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PythonOracleShape {
    ExactAssertion,
    BoundaryAssertion,
    ExceptionAssertion,
    FieldAssertion,
    OutputAssertion,
    StatusCodeAssertion,
    BroadSmokeAssertion,
    MockExpectation,
    UnknownCustomHelper,
}

impl PythonOracleShape {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExactAssertion => "exact_assertion",
            Self::BoundaryAssertion => "boundary_assertion",
            Self::ExceptionAssertion => "exception_assertion",
            Self::FieldAssertion => "field_assertion",
            Self::OutputAssertion => "output_assertion",
            Self::StatusCodeAssertion => "status_code_assertion",
            Self::BroadSmokeAssertion => "broad_smoke_assertion",
            Self::MockExpectation => "mock_expectation",
            Self::UnknownCustomHelper => "unknown_custom_helper",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonSourceFacts {
    file: PathBuf,
    language: &'static str,
    owners: Vec<PythonOwner>,
    tests: Vec<PythonTest>,
    facts: Vec<PythonSourceFact>,
    limitations: Vec<PythonSourceLimitation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonSourceFact {
    kind: PythonSourceFactKind,
    file: PathBuf,
    owner: Option<String>,
    start_line: usize,
    end_line: usize,
    start_byte: usize,
    end_byte: usize,
    text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PythonSourceFactKind {
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
    fn as_str(self) -> &'static str {
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
struct PythonSourceLimitation {
    kind: StaticLimitKind,
    evidence: String,
    missing: String,
}

fn parse_module_result(path: &Path, source: &str) -> Result<Mod, String> {
    let source_path = path.to_string_lossy();
    let module = parse(source, Mode::Module, source_path.as_ref())
        .map_err(|err| format!("parse_error: {err}"))?;
    match module {
        Mod::Module(_) => Ok(module),
        _ => Err("parse_error: expected Python module".to_string()),
    }
}

#[cfg(test)]
fn parse_module(path: &Path, source: &str) -> Option<Mod> {
    parse_module_result(path, source).ok()
}

fn extract_source_facts(file: &Path, source: &str) -> PythonSourceFacts {
    let mut snapshot = PythonSourceFacts {
        file: file.to_path_buf(),
        language: DomainLanguageId::Python.as_str(),
        owners: Vec::new(),
        tests: Vec::new(),
        facts: Vec::new(),
        limitations: Vec::new(),
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

    let imports = collect_imports_from_statements(&module.body);
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
    snapshot
}

fn source_fact_snapshot_observation(facts: &PythonSourceFacts) -> usize {
    let mut score = facts.file.components().count() + facts.language.len();
    score = score.saturating_add(facts.owners.len());
    score = score.saturating_add(facts.tests.len());
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

#[cfg(test)]
fn extract_owners(file: &Path, source: &str) -> Vec<PythonOwner> {
    extract_source_facts(file, source).owners
}

fn collect_owners_from_statements(
    file: &Path,
    source: &str,
    statements: &[Stmt],
    class_context: Option<&str>,
    imports: &[PythonImport],
    out: &mut Vec<PythonOwner>,
) {
    for stmt in statements {
        match stmt {
            Stmt::FunctionDef(function) => {
                out.push(owner_from_function(
                    PythonOwnerContext {
                        file,
                        source,
                        class_context,
                        imports,
                    },
                    function.name.as_str(),
                    function.range,
                    &function.decorator_list,
                    false,
                ));
            }
            Stmt::AsyncFunctionDef(function) => {
                out.push(owner_from_function(
                    PythonOwnerContext {
                        file,
                        source,
                        class_context,
                        imports,
                    },
                    function.name.as_str(),
                    function.range,
                    &function.decorator_list,
                    true,
                ));
            }
            Stmt::ClassDef(class) => {
                collect_owners_from_statements(
                    file,
                    source,
                    &class.body,
                    Some(class.name.as_str()),
                    imports,
                    out,
                );
                out.push(owner_from_class(
                    PythonOwnerContext {
                        file,
                        source,
                        class_context,
                        imports,
                    },
                    class.name.as_str(),
                    class.range,
                    &class.decorator_list,
                ));
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy)]
struct PythonOwnerContext<'a> {
    file: &'a Path,
    source: &'a str,
    class_context: Option<&'a str>,
    imports: &'a [PythonImport],
}

fn owner_from_function(
    context: PythonOwnerContext<'_>,
    name: &str,
    range: TextRange,
    decorators: &[Expr],
    is_async: bool,
) -> PythonOwner {
    let decorator_names = decorator_names(decorators);
    let owner_kind = if context.class_context.is_some()
        && decorator_names.iter().any(|decorator| {
            decorator.ends_with("classmethod") || decorator.ends_with("staticmethod")
        }) {
        OwnerKind::ClassMethod
    } else if context.class_context.is_some() {
        OwnerKind::Method
    } else {
        OwnerKind::Function
    };
    let qualified_name = context
        .class_context
        .map(|class| format!("{class}.{name}"))
        .unwrap_or_else(|| name.to_string());
    let mut decorators = decorator_names;
    if is_async {
        decorators.push("async_def".to_string());
    }
    PythonOwner {
        name: name.to_string(),
        qualified_name,
        file: context.file.to_path_buf(),
        start_line: line_for_range_start(context.source, range),
        end_line: line_for_range_end(context.source, range),
        owner_kind: Some(owner_kind),
        decorators,
        imports: context.imports.to_vec(),
    }
}

fn owner_from_class(
    context: PythonOwnerContext<'_>,
    name: &str,
    range: TextRange,
    decorators: &[Expr],
) -> PythonOwner {
    let qualified_name = context
        .class_context
        .map(|class| format!("{class}.{name}"))
        .unwrap_or_else(|| name.to_string());
    PythonOwner {
        name: name.to_string(),
        qualified_name,
        file: context.file.to_path_buf(),
        start_line: line_for_range_start(context.source, range),
        end_line: line_for_range_end(context.source, range),
        owner_kind: None,
        decorators: decorator_names(decorators),
        imports: context.imports.to_vec(),
    }
}

fn module_owner(
    file: &Path,
    source: &str,
    range: TextRange,
    imports: &[PythonImport],
) -> PythonOwner {
    PythonOwner {
        name: "<module>".to_string(),
        qualified_name: "<module>".to_string(),
        file: file.to_path_buf(),
        start_line: line_for_range_start(source, range),
        end_line: line_for_range_end(source, range),
        owner_kind: Some(OwnerKind::ModuleFunction),
        decorators: Vec::new(),
        imports: imports.to_vec(),
    }
}

#[cfg(test)]
fn extract_tests(file: &Path, source: &str) -> Vec<PythonTest> {
    extract_source_facts(file, source).tests
}

fn collect_tests_from_statements(
    file: &Path,
    source: &str,
    statements: &[Stmt],
    class_context: Option<&str>,
    in_unittest_class: bool,
    imports: &[PythonImport],
    out: &mut Vec<PythonTest>,
) {
    for stmt in statements {
        match stmt {
            Stmt::FunctionDef(function) if function.name.as_str().starts_with("test_") => {
                let framework = if in_unittest_class {
                    "unittest"
                } else {
                    "pytest"
                };
                let name = function.name.to_string();
                out.push(PythonTest {
                    qualified_name: qualified_test_name(class_context, &name),
                    name,
                    file: file.to_path_buf(),
                    line: line_for_range_start(source, function.range),
                    body_text: text_for_range(source, function.range),
                    imports: imports.to_vec(),
                    decorators: decorator_names(&function.decorator_list),
                    fixtures: fixture_parameter_names(&function.args, framework),
                    parametrized: is_parametrized(&function.decorator_list),
                    framework,
                    assertions: collect_assertions_from_statements(&function.body, source),
                });
            }
            Stmt::AsyncFunctionDef(function) if function.name.as_str().starts_with("test_") => {
                let framework = if in_unittest_class {
                    "unittest"
                } else {
                    "pytest"
                };
                let name = function.name.to_string();
                out.push(PythonTest {
                    qualified_name: qualified_test_name(class_context, &name),
                    name,
                    file: file.to_path_buf(),
                    line: line_for_range_start(source, function.range),
                    body_text: text_for_range(source, function.range),
                    imports: imports.to_vec(),
                    decorators: decorator_names(&function.decorator_list),
                    fixtures: fixture_parameter_names(&function.args, framework),
                    parametrized: is_parametrized(&function.decorator_list),
                    framework,
                    assertions: collect_assertions_from_statements(&function.body, source),
                });
            }
            Stmt::ClassDef(class) => {
                let class_is_unittest = is_unittest_class(class) || in_unittest_class;
                if class_is_unittest || is_pytest_class(class) {
                    let class_name = class.name.to_string();
                    let nested_class_context = qualified_test_name(class_context, &class_name);
                    collect_tests_from_statements(
                        file,
                        source,
                        &class.body,
                        Some(&nested_class_context),
                        class_is_unittest,
                        imports,
                        out,
                    );
                }
            }
            _ => {}
        }
    }
}

fn qualified_test_name(class_context: Option<&str>, name: &str) -> String {
    class_context
        .map(|class| format!("{class}.{name}"))
        .unwrap_or_else(|| name.to_string())
}

fn fixture_parameter_names(args: &ast::Arguments, framework: &str) -> Vec<String> {
    let mut names: Vec<String> = args
        .posonlyargs
        .iter()
        .chain(args.args.iter())
        .chain(args.kwonlyargs.iter())
        .map(|arg| arg.def.arg.to_string())
        .collect();
    if let Some(arg) = &args.vararg {
        names.push(arg.arg.to_string());
    }
    if let Some(arg) = &args.kwarg {
        names.push(arg.arg.to_string());
    }
    names.retain(|name| {
        name != "self"
            && name != "cls"
            && (framework == "pytest" || !matches!(name.as_str(), "subTest"))
    });
    names.sort();
    names.dedup();
    names
}

fn collect_imports_from_statements(statements: &[Stmt]) -> Vec<PythonImport> {
    let mut imports = Vec::new();
    for stmt in statements {
        match stmt {
            Stmt::Import(import) => {
                for alias in &import.names {
                    let imported = alias.name.to_string();
                    imports.push(PythonImport {
                        alias: alias
                            .asname
                            .as_ref()
                            .map(|name| name.to_string())
                            .unwrap_or_else(|| imported.clone()),
                        imported,
                    });
                }
            }
            Stmt::ImportFrom(import) => {
                for alias in &import.names {
                    let imported = alias.name.to_string();
                    imports.push(PythonImport {
                        alias: alias
                            .asname
                            .as_ref()
                            .map(|name| name.to_string())
                            .unwrap_or_else(|| imported.clone()),
                        imported,
                    });
                }
            }
            _ => {}
        }
    }
    imports
}

fn is_parametrized(decorators: &[Expr]) -> bool {
    decorator_names(decorators).iter().any(|decorator| {
        decorator == "parametrize"
            || decorator.ends_with(".parametrize")
            || decorator.ends_with("mark.parametrize")
    })
}

fn is_unittest_class(class: &ast::StmtClassDef) -> bool {
    class.bases.iter().any(|base| {
        expr_full_name(base).is_some_and(|name| name == "TestCase" || name.ends_with(".TestCase"))
    })
}

fn is_pytest_class(class: &ast::StmtClassDef) -> bool {
    class.name.as_str().starts_with("Test")
}

fn decorator_names(decorators: &[Expr]) -> Vec<String> {
    decorators.iter().filter_map(expr_full_name).collect()
}

fn expr_full_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.id.to_string()),
        Expr::Attribute(attribute) => expr_full_name(attribute.value.as_ref())
            .map(|prefix| format!("{prefix}.{}", attribute.attr)),
        Expr::Call(call) => expr_full_name(call.func.as_ref()),
        _ => None,
    }
}

fn collect_assertions_from_statements(statements: &[Stmt], source: &str) -> Vec<PythonAssertion> {
    let mut out = Vec::new();
    collect_assertions(statements, source, &mut out);
    out
}

fn collect_assertions(statements: &[Stmt], source: &str, out: &mut Vec<PythonAssertion>) {
    for stmt in statements {
        match stmt {
            Stmt::Assert(assert_stmt) => {
                out.push(assertion_from_assert(assert_stmt, source));
            }
            Stmt::Expr(expr_stmt) => {
                if let Some(assertion) = assertion_from_expr(expr_stmt.value.as_ref(), source) {
                    out.push(assertion);
                }
            }
            Stmt::If(if_stmt) => {
                collect_assertions(&if_stmt.body, source, out);
                collect_assertions(&if_stmt.orelse, source, out);
            }
            Stmt::For(for_stmt) => {
                collect_assertions(&for_stmt.body, source, out);
                collect_assertions(&for_stmt.orelse, source, out);
            }
            Stmt::AsyncFor(for_stmt) => {
                collect_assertions(&for_stmt.body, source, out);
                collect_assertions(&for_stmt.orelse, source, out);
            }
            Stmt::While(while_stmt) => {
                collect_assertions(&while_stmt.body, source, out);
                collect_assertions(&while_stmt.orelse, source, out);
            }
            Stmt::With(with_stmt) => {
                collect_with_item_assertions(&with_stmt.items, source, out);
                collect_assertions(&with_stmt.body, source, out);
            }
            Stmt::AsyncWith(with_stmt) => {
                collect_with_item_assertions(&with_stmt.items, source, out);
                collect_assertions(&with_stmt.body, source, out);
            }
            Stmt::Try(try_stmt) => {
                collect_assertions(&try_stmt.body, source, out);
                collect_except_handler_assertions(&try_stmt.handlers, source, out);
                collect_assertions(&try_stmt.orelse, source, out);
                collect_assertions(&try_stmt.finalbody, source, out);
            }
            Stmt::TryStar(try_stmt) => {
                collect_assertions(&try_stmt.body, source, out);
                collect_except_handler_assertions(&try_stmt.handlers, source, out);
                collect_assertions(&try_stmt.orelse, source, out);
                collect_assertions(&try_stmt.finalbody, source, out);
            }
            Stmt::Match(match_stmt) => {
                for case in &match_stmt.cases {
                    collect_assertions(&case.body, source, out);
                }
            }
            _ => {}
        }
    }
}

fn collect_with_item_assertions(
    items: &[ast::WithItem],
    source: &str,
    out: &mut Vec<PythonAssertion>,
) {
    for item in items {
        if let Some(assertion) = assertion_from_expr(&item.context_expr, source) {
            out.push(assertion);
        }
    }
}

fn collect_except_handler_assertions(
    handlers: &[ast::ExceptHandler],
    source: &str,
    out: &mut Vec<PythonAssertion>,
) {
    for handler in handlers {
        let ast::ExceptHandler::ExceptHandler(handler) = handler;
        collect_assertions(&handler.body, source, out);
    }
}

fn assertion_from_assert(assert_stmt: &ast::StmtAssert, source: &str) -> PythonAssertion {
    let (oracle_kind, oracle_strength, oracle_shape) =
        oracle_for_assert_expr(assert_stmt.test.as_ref());
    PythonAssertion {
        text: text_for_range(source, assert_stmt.range).trim().to_string(),
        line: line_for_range_start(source, assert_stmt.range),
        oracle_kind,
        oracle_strength,
        oracle_shape,
    }
}

fn assertion_from_expr(expr: &Expr, source: &str) -> Option<PythonAssertion> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let (oracle_kind, oracle_strength, oracle_shape) = oracle_for_call(call)?;
    Some(PythonAssertion {
        text: text_for_range(source, call.range).trim().to_string(),
        line: line_for_range_start(source, call.range),
        oracle_kind,
        oracle_strength,
        oracle_shape,
    })
}

fn oracle_for_assert_expr(expr: &Expr) -> (OracleKind, OracleStrength, PythonOracleShape) {
    match expr {
        Expr::Compare(compare) => oracle_for_compare(compare),
        Expr::Call(call) => {
            if expr_full_name(call.func.as_ref()).is_some_and(|name| name == "isinstance") {
                (
                    OracleKind::RelationalCheck,
                    OracleStrength::Weak,
                    PythonOracleShape::BoundaryAssertion,
                )
            } else {
                oracle_for_call(call).unwrap_or((
                    OracleKind::SmokeOnly,
                    OracleStrength::Smoke,
                    PythonOracleShape::BroadSmokeAssertion,
                ))
            }
        }
        _ => (
            OracleKind::SmokeOnly,
            OracleStrength::Smoke,
            PythonOracleShape::BroadSmokeAssertion,
        ),
    }
}

fn oracle_for_compare(
    compare: &ast::ExprCompare,
) -> (OracleKind, OracleStrength, PythonOracleShape) {
    let has_exact = compare.ops.iter().any(|op| matches!(op, ast::CmpOp::Eq));
    let (kind, strength) = if has_exact {
        (OracleKind::ExactValue, OracleStrength::Strong)
    } else {
        (OracleKind::RelationalCheck, OracleStrength::Weak)
    };
    let shape = if compare_observes_output(compare) {
        PythonOracleShape::OutputAssertion
    } else if compare_observes_status_code(compare) {
        PythonOracleShape::StatusCodeAssertion
    } else if compare_observes_field(compare) {
        PythonOracleShape::FieldAssertion
    } else if compare.ops.iter().any(|op| {
        matches!(
            op,
            ast::CmpOp::Lt | ast::CmpOp::LtE | ast::CmpOp::Gt | ast::CmpOp::GtE
        )
    }) {
        PythonOracleShape::BoundaryAssertion
    } else if has_exact {
        PythonOracleShape::ExactAssertion
    } else {
        PythonOracleShape::BoundaryAssertion
    };
    (kind, strength, shape)
}

fn compare_observes_output(compare: &ast::ExprCompare) -> bool {
    expr_observes_output(compare.left.as_ref())
        || compare.comparators.iter().any(expr_observes_output)
}

fn compare_observes_status_code(compare: &ast::ExprCompare) -> bool {
    expr_observes_status_code(compare.left.as_ref())
        || compare.comparators.iter().any(expr_observes_status_code)
}

fn compare_observes_field(compare: &ast::ExprCompare) -> bool {
    expr_observes_field(compare.left.as_ref())
        || compare.comparators.iter().any(expr_observes_field)
}

fn expr_observes_output(expr: &Expr) -> bool {
    expr_full_name(expr).is_some_and(|name| {
        name == "caplog.text"
            || name == "capsys.readouterr.out"
            || name.ends_with(".output")
            || name.ends_with(".stdout")
            || name.ends_with(".stderr")
            || name.ends_with(".text")
    }) || match expr {
        Expr::Call(call) => {
            expr_full_name(call.func.as_ref()).is_some_and(|name| name == "capsys.readouterr")
                || call.args.iter().any(expr_observes_output)
                || call
                    .keywords
                    .iter()
                    .any(|keyword| expr_observes_output(&keyword.value))
        }
        Expr::Attribute(attribute) => expr_observes_output(attribute.value.as_ref()),
        Expr::Subscript(subscript) => {
            expr_observes_output(subscript.value.as_ref())
                || expr_observes_output(subscript.slice.as_ref())
        }
        Expr::BoolOp(bool_op) => bool_op.values.iter().any(expr_observes_output),
        _ => false,
    }
}

fn expr_observes_status_code(expr: &Expr) -> bool {
    expr_full_name(expr).is_some_and(|name| {
        name.ends_with(".status_code") || name.ends_with(".status") || name.ends_with(".exit_code")
    })
}

fn expr_observes_field(expr: &Expr) -> bool {
    match expr {
        Expr::Attribute(attribute) => {
            !expr_observes_status_code(expr)
                && !expr_observes_output(expr)
                && !expr_observes_output(attribute.value.as_ref())
        }
        Expr::Subscript(_) => true,
        Expr::Call(call) => {
            call.args.iter().any(expr_observes_field)
                || call
                    .keywords
                    .iter()
                    .any(|keyword| expr_observes_field(&keyword.value))
        }
        Expr::BoolOp(bool_op) => bool_op.values.iter().any(expr_observes_field),
        _ => false,
    }
}

fn oracle_for_call(
    call: &ast::ExprCall,
) -> Option<(OracleKind, OracleStrength, PythonOracleShape)> {
    let name = expr_full_name(call.func.as_ref())?;
    let last_segment = name.rsplit('.').next().unwrap_or(name.as_str());
    match last_segment {
        "assertEqual" => Some((
            OracleKind::ExactValue,
            OracleStrength::Strong,
            oracle_shape_for_call_arguments(call, PythonOracleShape::ExactAssertion),
        )),
        "assertDictEqual" => Some((
            OracleKind::ExactValue,
            OracleStrength::Strong,
            oracle_shape_for_call_arguments(call, PythonOracleShape::FieldAssertion),
        )),
        "assertIn" | "assertRegex" => Some((
            OracleKind::RelationalCheck,
            OracleStrength::Weak,
            oracle_shape_for_call_arguments(call, PythonOracleShape::FieldAssertion),
        )),
        "assertNotEqual" => Some((
            OracleKind::RelationalCheck,
            OracleStrength::Weak,
            oracle_shape_for_call_arguments(call, PythonOracleShape::BoundaryAssertion),
        )),
        "assertTrue" | "assertFalse" => Some((
            OracleKind::SmokeOnly,
            OracleStrength::Smoke,
            PythonOracleShape::BroadSmokeAssertion,
        )),
        "assertRaises" | "assertRaisesRegex" => Some((
            OracleKind::BroadError,
            OracleStrength::Weak,
            PythonOracleShape::ExceptionAssertion,
        )),
        "raises" if name == "pytest.raises" || name == "raises" => Some((
            OracleKind::BroadError,
            OracleStrength::Weak,
            PythonOracleShape::ExceptionAssertion,
        )),
        "assert_called"
        | "assert_called_once"
        | "assert_called_with"
        | "assert_called_once_with"
        | "assert_any_call"
        | "assert_has_calls"
        | "assert_not_called" => Some((
            OracleKind::MockExpectation,
            OracleStrength::Medium,
            PythonOracleShape::MockExpectation,
        )),
        _ if looks_like_custom_assertion_helper(&name) => Some((
            OracleKind::Unknown,
            OracleStrength::Unknown,
            PythonOracleShape::UnknownCustomHelper,
        )),
        _ => None,
    }
}

fn oracle_shape_for_call_arguments(
    call: &ast::ExprCall,
    fallback: PythonOracleShape,
) -> PythonOracleShape {
    if call.args.iter().any(expr_observes_output)
        || call
            .keywords
            .iter()
            .any(|keyword| expr_observes_output(&keyword.value))
    {
        PythonOracleShape::OutputAssertion
    } else if call.args.iter().any(expr_observes_status_code)
        || call
            .keywords
            .iter()
            .any(|keyword| expr_observes_status_code(&keyword.value))
    {
        PythonOracleShape::StatusCodeAssertion
    } else if call.args.iter().any(expr_observes_field)
        || call
            .keywords
            .iter()
            .any(|keyword| expr_observes_field(&keyword.value))
    {
        PythonOracleShape::FieldAssertion
    } else {
        fallback
    }
}

fn looks_like_custom_assertion_helper(name: &str) -> bool {
    name.rsplit('.')
        .next()
        .is_some_and(|segment| segment.starts_with("assert_") || segment == "assert_that")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PythonRelationKind {
    SyntacticCall,
    ImportAliasCall,
    SameStem,
    TestNameSimilarity,
    FixtureName,
}

impl PythonRelationKind {
    fn rank(self) -> u8 {
        match self {
            Self::SyntacticCall => 5,
            Self::ImportAliasCall => 4,
            Self::SameStem => 3,
            Self::TestNameSimilarity => 2,
            Self::FixtureName => 1,
        }
    }

    fn uses_oracle(self) -> bool {
        matches!(self, Self::SyntacticCall | Self::ImportAliasCall)
    }

    fn is_uncertain(self) -> bool {
        !self.uses_oracle()
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::SyntacticCall => "syntactic_call",
            Self::ImportAliasCall => "import_alias_call",
            Self::SameStem => "same_stem",
            Self::TestNameSimilarity => "test_name_similarity",
            Self::FixtureName => "fixture_name",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PythonRelatedCandidate<'a> {
    test: &'a PythonTest,
    relation: PythonRelationKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonRepairPlacement {
    repair_action: &'static str,
    suggested_test_file: String,
    suggested_test_name: String,
    suggested_test_node_id: Option<String>,
    verify_command: String,
    verify_command_confidence: &'static str,
    location_reason: &'static str,
}

fn related_test_candidates<'a>(
    owner: &PythonOwner,
    all_tests: &'a [PythonTest],
) -> Vec<PythonRelatedCandidate<'a>> {
    let mut candidates: Vec<PythonRelatedCandidate<'a>> = all_tests
        .iter()
        .filter_map(|test| {
            related_test_relation(test, owner)
                .map(|relation| PythonRelatedCandidate { test, relation })
        })
        .collect();
    candidates.sort_by(|left, right| {
        right
            .relation
            .rank()
            .cmp(&left.relation.rank())
            .then_with(|| {
                let left_rank = strongest_assertion(&left.test.assertions)
                    .map(|assertion| assertion.oracle_strength.rank())
                    .unwrap_or(0);
                let right_rank = strongest_assertion(&right.test.assertions)
                    .map(|assertion| assertion.oracle_strength.rank())
                    .unwrap_or(0);
                right_rank.cmp(&left_rank)
            })
            .then_with(|| left.test.file.cmp(&right.test.file))
            .then_with(|| left.test.name.cmp(&right.test.name))
    });
    candidates
}

fn find_related_tests(owner: &PythonOwner, all_tests: &[PythonTest]) -> Vec<RelatedTest> {
    related_test_candidates(owner, all_tests)
        .into_iter()
        .map(|candidate| {
            let strongest = candidate
                .relation
                .uses_oracle()
                .then(|| strongest_assertion(&candidate.test.assertions))
                .flatten();
            let (oracle_kind, oracle_strength, oracle) = match strongest {
                Some(assertion) => (
                    assertion.oracle_kind.clone(),
                    assertion.oracle_strength.clone(),
                    Some(assertion.text.clone()),
                ),
                None if candidate.relation.uses_oracle() && candidate.test.parametrized => (
                    OracleKind::Unknown,
                    OracleStrength::Unknown,
                    Some("pytest.mark.parametrize".to_string()),
                ),
                None => (OracleKind::Unknown, OracleStrength::Unknown, None),
            };
            RelatedTest {
                name: candidate.test.name.clone(),
                file: candidate.test.file.clone(),
                line: candidate.test.line,
                oracle,
                oracle_kind,
                oracle_strength,
            }
        })
        .collect()
}

fn verify_command_for_test(test: &PythonTest) -> Option<String> {
    let path = normalized_path(&test.file);
    match test.framework {
        "pytest" => {
            let node = test.qualified_name.replace('.', "::");
            Some(format!("pytest {path}::{node}"))
        }
        "unittest" => {
            let module = path
                .strip_suffix(".py")
                .unwrap_or(path.as_str())
                .replace('/', ".");
            Some(format!(
                "python -m unittest {module}.{}",
                test.qualified_name
            ))
        }
        _ => None,
    }
}

fn unittest_module_for_path(path: &str) -> String {
    path.strip_suffix(".py")
        .unwrap_or(path)
        .replace(['/', '\\'], ".")
}

fn python_repair_placement(
    class: &ExposureClass,
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> Option<PythonRepairPlacement> {
    if !matches!(class, ExposureClass::WeaklyExposed) {
        return None;
    }
    let candidate = related_candidates
        .iter()
        .find(|candidate| candidate.relation.uses_oracle())?;
    let path = normalized_path(&candidate.test.file);
    match candidate.test.framework {
        "pytest" => {
            let node_id = format!(
                "{path}::{}",
                candidate.test.qualified_name.replace('.', "::")
            );
            Some(PythonRepairPlacement {
                repair_action: "strengthen_existing_test",
                suggested_test_file: path,
                suggested_test_name: candidate.test.name.clone(),
                suggested_test_node_id: Some(node_id.clone()),
                verify_command: format!("pytest {node_id}"),
                verify_command_confidence: "high",
                location_reason: "strengthen existing weak pytest relation",
            })
        }
        "unittest" => {
            let selector = format!(
                "{}.{}",
                unittest_module_for_path(&path),
                candidate.test.qualified_name
            );
            Some(PythonRepairPlacement {
                repair_action: "strengthen_existing_test",
                suggested_test_file: path,
                suggested_test_name: candidate.test.name.clone(),
                suggested_test_node_id: None,
                verify_command: format!("python -m unittest {selector}"),
                verify_command_confidence: "high",
                location_reason: "strengthen existing weak unittest relation",
            })
        }
        _ => None,
    }
}

fn strongest_assertion(assertions: &[PythonAssertion]) -> Option<&PythonAssertion> {
    assertions
        .iter()
        .max_by_key(|assertion| assertion.oracle_strength.rank())
}

fn related_test_relation(test: &PythonTest, owner: &PythonOwner) -> Option<PythonRelationKind> {
    if body_calls_owner(&test.body_text, owner) {
        return Some(PythonRelationKind::SyntacticCall);
    }
    if import_alias_calls_owner(test, owner) {
        return Some(PythonRelationKind::ImportAliasCall);
    }
    if same_stem_related(test, owner) {
        return Some(PythonRelationKind::SameStem);
    }
    if test_name_similar_to_owner(test, owner) {
        return Some(PythonRelationKind::TestNameSimilarity);
    }
    if fixture_name_related_to_owner(test, owner) {
        return Some(PythonRelationKind::FixtureName);
    }
    None
}

fn body_calls_owner(body_text: &str, owner: &PythonOwner) -> bool {
    contains_call_name(body_text, &owner.name)
        || (owner.qualified_name != owner.name
            && contains_call_name(body_text, &owner.qualified_name))
        || (matches!(
            owner.owner_kind,
            Some(OwnerKind::Method | OwnerKind::ClassMethod)
        ) && contains_any_attribute_call(body_text, &owner.name))
}

fn import_alias_calls_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    test.imports.iter().any(|import| {
        (import.imported == owner.name
            && import.alias != owner.name
            && contains_call_name(&test.body_text, &import.alias))
            || (imported_module_matches_owner(import, owner)
                && contains_attribute_call(&test.body_text, &import.alias, &owner.name))
    })
}

fn imported_module_matches_owner(import: &PythonImport, owner: &PythonOwner) -> bool {
    owner
        .file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| import.imported.rsplit('.').next() == Some(stem))
}

fn contains_call_name(body_text: &str, call_name: &str) -> bool {
    let needle = format!("{call_name}(");
    body_text.match_indices(&needle).any(|(idx, _)| {
        has_call_boundary(body_text, idx)
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
    })
}

fn contains_attribute_call(body_text: &str, receiver: &str, attr: &str) -> bool {
    let needle = format!("{receiver}.{attr}(");
    body_text.match_indices(&needle).any(|(idx, _)| {
        has_call_boundary(body_text, idx)
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
    })
}

fn contains_any_attribute_call(body_text: &str, attr: &str) -> bool {
    let needle = format!(".{attr}(");
    body_text
        .match_indices(&needle)
        .any(|(idx, _)| !line_prefix_looks_like_comment_or_string(body_text, idx))
}

fn has_call_boundary(body_text: &str, idx: usize) -> bool {
    body_text[..idx]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_python_identifier_char(ch) && ch != '.')
}

fn line_prefix_looks_like_comment_or_string(body_text: &str, idx: usize) -> bool {
    let line_start = body_text[..idx].rfind('\n').map_or(0, |offset| offset + 1);
    let prefix = &body_text[line_start..idx];
    prefix.trim_start().starts_with('#') || has_unclosed_quote(prefix)
}

fn has_unclosed_quote(prefix: &str) -> bool {
    let mut escaped = false;
    let mut in_single = false;
    let mut in_double = false;
    for ch in prefix.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if ch == '"' && !in_single {
            in_double = !in_double;
        }
    }
    in_single || in_double
}

fn is_python_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn same_stem_related(test: &PythonTest, owner: &PythonOwner) -> bool {
    let Some(owner_stem) = owner.file.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let Some(test_stem) = test.file.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    normalize_test_stem(test_stem) == owner_stem
}

fn normalize_test_stem(stem: &str) -> &str {
    stem.strip_prefix("test_")
        .or_else(|| stem.strip_suffix("_test"))
        .unwrap_or(stem)
}

fn test_name_similar_to_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    let test_key = normalize_similarity_key(&test.name);
    owner_similarity_keys(owner)
        .into_iter()
        .any(|key| similarity_key_contains(&test_key, &key))
}

fn fixture_name_related_to_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    test.fixtures.iter().any(|fixture| {
        let fixture_key = normalize_similarity_key(fixture);
        owner_similarity_keys(owner)
            .into_iter()
            .any(|key| similarity_key_contains(&fixture_key, &key))
    })
}

fn owner_similarity_keys(owner: &PythonOwner) -> Vec<String> {
    let mut keys = Vec::new();
    if !owner.is_module_owner() {
        keys.push(normalize_similarity_key(&owner.name));
        if owner.qualified_name != owner.name {
            keys.push(normalize_similarity_key(
                &owner.qualified_name.replace('.', "_"),
            ));
        }
    }
    if let Some(stem) = owner.file.file_stem().and_then(|stem| stem.to_str()) {
        keys.push(normalize_similarity_key(stem));
    }
    keys.sort();
    keys.dedup();
    keys.into_iter().filter(|key| key.len() >= 4).collect()
}

fn normalize_similarity_key(text: &str) -> String {
    let mut out = String::new();
    let mut last_was_separator = true;
    for ch in text.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_was_separator = false;
        } else if !last_was_separator {
            out.push('_');
            last_was_separator = true;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

fn similarity_key_contains(haystack: &str, needle: &str) -> bool {
    if haystack.is_empty() || needle.is_empty() {
        return false;
    }
    haystack == needle
        || haystack
            .strip_prefix(needle)
            .is_some_and(|tail| tail.starts_with('_'))
        || haystack
            .strip_suffix(needle)
            .is_some_and(|head| head.ends_with('_'))
        || haystack.contains(&format!("_{needle}_"))
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonStaticLimit {
    kind: StaticLimitKind,
    evidence: String,
    missing: String,
}

fn static_limit_for_change(
    line_text: &str,
    owner: &PythonOwner,
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> Option<PythonStaticLimit> {
    let trimmed = line_text.trim();
    if contains_dynamic_dispatch(trimmed) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::DynamicDispatch,
            evidence: "static_limit dynamic_dispatch: changed line uses dynamic call dispatch"
                .to_string(),
            missing: "Static limit `dynamic_dispatch`: the Python preview adapter saw a dynamic call shape such as `getattr(...)` or `registry[key](...)`; syntax alone cannot resolve the called behavior.".to_string(),
        });
    }
    if contains_metaprogramming(trimmed) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::Metaprogramming,
            evidence: "static_limit metaprogramming: changed line uses metaprogramming syntax"
                .to_string(),
            missing: "Static limit `metaprogramming`: the Python preview adapter saw metaprogramming syntax and does not infer runtime-created behavior.".to_string(),
        });
    }
    if let Some(decorator) = owner
        .decorators
        .iter()
        .find(|decorator| !is_transparent_owner_decorator(decorator))
    {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::DecoratorIndirection,
            evidence: format!("static_limit decorator_indirection: `{decorator}`"),
            missing: format!(
                "Static limit `decorator_indirection`: owner `{}` is decorated with `{decorator}`; syntax-first preview evidence does not resolve decorator-modified call behavior.",
                owner.qualified_name
            ),
        });
    }
    if related_candidates
        .iter()
        .any(|candidate| test_has_mocked_module(candidate.test))
    {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::MockedModule,
            evidence: "static_limit mocked_module: related test uses patch/mock module syntax"
                .to_string(),
            missing: "Static limit `mocked_module`: a related Python test uses patch/mock-module syntax; the preview adapter does not resolve runtime substitution semantics.".to_string(),
        });
    }
    if related_candidates_have_opaque_custom_assertion_limit(related_candidates) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::OpaqueCustomAssertionHelper,
            evidence: "static_limit opaque_custom_assertion_helper: related test uses an opaque custom assertion helper"
                .to_string(),
            missing: "Static limit `opaque_custom_assertion_helper`: a related Python test uses a custom assertion helper such as `assert_*(...)`; the preview adapter cannot inspect the helper body or determine whether it already observes the changed discriminator.".to_string(),
        });
    }
    if line_uses_imported_symbol(trimmed, &owner.imports) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::MissingImportGraph,
            evidence: "static_limit missing_import_graph: changed line calls an imported symbol"
                .to_string(),
            missing: "Static limit `missing_import_graph`: the changed line calls an imported symbol; the Python preview adapter does not build an import graph or resolve imported implementation semantics.".to_string(),
        });
    }
    if trimmed.contains("lambda ") {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::UnsupportedSyntax,
            evidence: "static_limit unsupported_syntax: changed line uses lambda syntax"
                .to_string(),
            missing: "Static limit `unsupported_syntax`: the changed line uses a Python syntax shape this preview adapter does not model precisely yet.".to_string(),
        });
    }
    None
}

fn contains_dynamic_dispatch(text: &str) -> bool {
    text.contains("getattr(") || (text.contains('[') && text.contains("]("))
}

fn contains_metaprogramming(text: &str) -> bool {
    text.contains("__getattr__") || text.contains("type(") || text.contains("setattr(")
}

fn is_transparent_owner_decorator(decorator: &str) -> bool {
    decorator == "staticmethod" || decorator == "classmethod" || decorator == "async_def"
}

fn test_has_mocked_module(test: &PythonTest) -> bool {
    test.decorators
        .iter()
        .any(|decorator| decorator == "patch" || decorator.ends_with(".patch"))
        || test.body_text.contains("patch(")
        || test.body_text.contains(".patch(")
        || test.body_text.contains("monkeypatch.setattr(")
        || test.body_text.contains("monkeypatch.setitem(")
        || test.body_text.contains("monkeypatch.delattr(")
}

fn related_candidates_have_opaque_custom_assertion_limit(
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> bool {
    let mut has_opaque_helper = false;
    let mut has_known_strong_oracle = false;

    for candidate in related_candidates
        .iter()
        .filter(|candidate| candidate.relation.uses_oracle())
    {
        for assertion in &candidate.test.assertions {
            if assertion.oracle_shape == PythonOracleShape::UnknownCustomHelper {
                has_opaque_helper = true;
            } else if assertion.oracle_strength.rank() >= OracleStrength::Strong.rank() {
                has_known_strong_oracle = true;
            }
        }
    }

    has_opaque_helper && !has_known_strong_oracle
}

fn line_uses_imported_symbol(text: &str, imports: &[PythonImport]) -> bool {
    imports.iter().any(|import| {
        !is_known_mock_constructor_import(import)
            && (text.contains(&format!("{}(", import.alias))
                || text.contains(&format!("{}.", import.alias)))
    })
}

fn is_known_mock_constructor_import(import: &PythonImport) -> bool {
    matches!(import.imported.as_str(), "Mock" | "MagicMock")
        || matches!(import.alias.as_str(), "Mock" | "MagicMock")
}

fn classify_probe_shape(line_text: &str) -> (ProbeFamily, DeltaKind) {
    let trimmed = line_text.trim_start();
    if (trimmed.contains(" if ") && trimmed.contains(" else "))
        || trimmed.starts_with("if ")
        || trimmed.starts_with("elif ")
        || trimmed.starts_with("while ")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("match ")
        || trimmed.starts_with("case ")
    {
        return (ProbeFamily::Predicate, DeltaKind::Control);
    }
    if trimmed.starts_with("raise ")
        || trimmed == "raise"
        || trimmed.starts_with("try:")
        || trimmed.starts_with("except ")
        || trimmed.starts_with("except* ")
        || trimmed.starts_with("finally:")
        || (trimmed.starts_with("with ") && trimmed.contains("raises("))
    {
        return (ProbeFamily::ErrorPath, DeltaKind::Control);
    }
    if python_return_dict_field_discriminator(trimmed).is_some() {
        return (ProbeFamily::FieldConstruction, DeltaKind::Value);
    }
    if trimmed.starts_with("return ") || trimmed == "return" {
        return (ProbeFamily::ReturnValue, DeltaKind::Value);
    }
    if contains_mock_initializer(trimmed) {
        return (ProbeFamily::SideEffect, DeltaKind::Effect);
    }
    if let Some(eq_idx) = trimmed.find('=')
        && !trimmed.contains("==")
        && !trimmed.contains("!=")
        && !trimmed.contains(">=")
        && !trimmed.contains("<=")
    {
        let lhs = trimmed[..eq_idx].trim();
        if lhs.contains('.')
            && lhs.chars().all(|ch| {
                ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '[' || ch == ']'
            })
        {
            return (ProbeFamily::FieldConstruction, DeltaKind::Value);
        }
        let rhs = trimmed[eq_idx + 1..].trim();
        if looks_like_call_expression(rhs) {
            return (ProbeFamily::SideEffect, DeltaKind::Effect);
        }
    }
    let call_candidate = trimmed.strip_prefix("await ").unwrap_or(trimmed).trim_end();
    if looks_like_call_expression(call_candidate)
        && !call_candidate.starts_with("assert ")
        && !call_candidate.starts_with("def ")
        && !call_candidate.starts_with("class ")
        && !call_candidate.starts_with("with ")
    {
        return (ProbeFamily::SideEffect, DeltaKind::Effect);
    }
    (ProbeFamily::Predicate, DeltaKind::Control)
}

fn contains_mock_initializer(text: &str) -> bool {
    text.contains("Mock(") || text.contains("MagicMock(")
}

fn looks_like_call_expression(text: &str) -> bool {
    let text = text.trim_end_matches(';').trim_end();
    text.contains('(') && text.ends_with(')')
}

fn canonical_python_gap_for(
    file: &Path,
    owner: &PythonOwner,
    probe_family: &ProbeFamily,
    line_text: &str,
) -> FindingCanonicalGap {
    let file = normalized_path(file);
    let behavior_kind = python_behavior_kind(probe_family).to_string();
    let probe_kind = probe_family.as_str().to_string();
    let normalized_discriminator = normalize_python_gap_discriminator(probe_family, line_text);
    let id = format!(
        "gap:python:{file}:{}:{behavior_kind}:{probe_kind}:{normalized_discriminator}",
        owner.qualified_name
    );

    FindingCanonicalGap {
        id,
        language: "python".to_string(),
        file,
        owner: owner.qualified_name.clone(),
        behavior_kind,
        probe_kind,
        normalized_discriminator,
    }
}

fn python_behavior_kind(probe_family: &ProbeFamily) -> &'static str {
    match probe_family {
        ProbeFamily::Predicate => "predicate_boundary",
        ProbeFamily::ReturnValue => "return_value",
        ProbeFamily::ErrorPath => "exception_path",
        ProbeFamily::FieldConstruction => "field_value",
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => "call_or_output_effect",
        ProbeFamily::MatchArm => "match_arm",
        ProbeFamily::StaticUnknown => "static_unknown",
    }
}

fn normalize_python_gap_discriminator(probe_family: &ProbeFamily, line_text: &str) -> String {
    let mut text = line_text.trim().trim_end_matches(';').trim().to_string();
    match probe_family {
        ProbeFamily::Predicate => {
            for prefix in ["if ", "elif ", "while ", "for ", "match ", "case "] {
                if let Some(stripped) = text.strip_prefix(prefix) {
                    text = stripped.to_string();
                    break;
                }
            }
            text = text.trim_end_matches(':').trim().to_string();
        }
        ProbeFamily::ReturnValue => {
            if let Some(stripped) = text.strip_prefix("return ") {
                text = stripped.to_string();
            }
        }
        ProbeFamily::ErrorPath => {
            if let Some(stripped) = text.strip_prefix("raise ") {
                text = stripped.to_string();
            }
            text = text.trim_end_matches(':').trim().to_string();
        }
        _ => {}
    }
    normalize_gap_key_text(&text)
}

fn normalize_gap_key_text(text: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_word = false;
    let mut pending_separator = false;

    for character in text.chars() {
        if character.is_ascii_alphanumeric() || character == '_' || character == '.' {
            if pending_separator && previous_was_word {
                normalized.push('_');
            }
            normalized.push(character.to_ascii_lowercase());
            previous_was_word = true;
            pending_separator = false;
        } else if matches!(
            character,
            '=' | '!' | '<' | '>' | '+' | '-' | '*' | '/' | '%' | '[' | ']'
        ) {
            normalized.push(character);
            previous_was_word = false;
            pending_separator = false;
        } else {
            pending_separator = true;
        }
    }

    let trimmed = normalized.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "unknown".to_string()
    } else {
        trimmed
    }
}

fn python_infection_evidence(probe_family: &ProbeFamily, line_text: &str) -> StageEvidence {
    let summary = match probe_family {
        ProbeFamily::Predicate => {
            if is_python_control_predicate_line(line_text) {
                format!(
                    "Changed Python predicate can alter branch selection: `{}`",
                    line_text.trim()
                )
            } else {
                format!(
                    "Changed Python expression can alter preview-classified predicate behavior: `{}`",
                    line_text.trim()
                )
            }
        }
        ProbeFamily::ReturnValue => {
            format!(
                "Changed Python return expression can alter the owner return value: `{}`",
                line_text.trim()
            )
        }
        ProbeFamily::ErrorPath => {
            format!(
                "Changed Python error path can alter raised exception/control behavior: `{}`",
                line_text.trim()
            )
        }
        ProbeFamily::FieldConstruction => {
            format!(
                "Changed Python field or attribute construction can alter object state: `{}`",
                line_text.trim()
            )
        }
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
            format!(
                "Changed Python call or output effect can alter observable side effects: `{}`",
                line_text.trim()
            )
        }
        ProbeFamily::MatchArm => {
            format!(
                "Changed Python match arm can alter selected branch behavior: `{}`",
                line_text.trim()
            )
        }
        ProbeFamily::StaticUnknown => {
            "Python preview could not classify the changed behavior shape.".to_string()
        }
    };
    StageEvidence::new(StageState::Yes, Confidence::Low, summary)
}

fn python_propagation_evidence(
    probe_family: &ProbeFamily,
    line_text: &str,
    static_limit: Option<&PythonStaticLimit>,
) -> StageEvidence {
    if let Some(limit) = static_limit {
        return StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            format!(
                "Static limit `{}` prevents a safe Python propagation claim.",
                limit.kind.as_str()
            ),
        );
    }

    match probe_family {
        ProbeFamily::ReturnValue => StageEvidence::new(
            StageState::Yes,
            Confidence::Low,
            "Changed Python return value is already at the owner output boundary.",
        ),
        ProbeFamily::ErrorPath => StageEvidence::new(
            StageState::Yes,
            Confidence::Low,
            "Changed Python error path propagates through the exception/control boundary.",
        ),
        ProbeFamily::FieldConstruction => StageEvidence::new(
            StageState::Weak,
            Confidence::Low,
            "Changed Python field construction can propagate through returned or retained object state; exact runtime object flow is not resolved.",
        ),
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => StageEvidence::new(
            StageState::Weak,
            Confidence::Low,
            "Changed Python call/output behavior can propagate through side effects; runtime target resolution is not inferred.",
        ),
        ProbeFamily::Predicate | ProbeFamily::MatchArm => {
            let summary = if matches!(probe_family, ProbeFamily::Predicate)
                && !is_python_control_predicate_line(line_text)
            {
                "Changed Python fallback expression can propagate through selected behavior; preview evidence does not prove the concrete downstream sink."
            } else if matches!(probe_family, ProbeFamily::Predicate) {
                "Changed Python control flow can propagate by selecting a different branch; preview evidence does not prove the concrete downstream sink."
            } else {
                "Changed Python match arm can propagate by selecting a different branch; preview evidence does not prove the concrete downstream sink."
            };
            StageEvidence::new(StageState::Weak, Confidence::Low, summary)
        }
        ProbeFamily::StaticUnknown => StageEvidence::new(
            StageState::Unknown,
            Confidence::Low,
            "Python preview could not classify a propagation path for this changed behavior.",
        ),
    }
}

fn python_flow_sink_for(
    probe_family: &ProbeFamily,
    owner: &PythonOwner,
    line: usize,
    line_text: &str,
) -> Option<FlowSinkFact> {
    let kind = match probe_family {
        ProbeFamily::ReturnValue => FlowSinkKind::ReturnValue,
        ProbeFamily::ErrorPath => FlowSinkKind::ErrorVariant,
        ProbeFamily::FieldConstruction => FlowSinkKind::StructField,
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => FlowSinkKind::CallEffect,
        ProbeFamily::Predicate | ProbeFamily::MatchArm => FlowSinkKind::Unknown,
        ProbeFamily::StaticUnknown => return None,
    };

    Some(FlowSinkFact {
        kind,
        text: line_text.trim().to_string(),
        line,
        owner: Some(owner.symbol_id()),
    })
}

fn python_missing_discriminators(
    probe_family: &ProbeFamily,
    line: usize,
    line_text: &str,
    flow_sink: Option<&FlowSinkFact>,
) -> Vec<MissingDiscriminatorFact> {
    let Some(value) = python_missing_discriminator_value(probe_family, line_text) else {
        return Vec::new();
    };

    vec![MissingDiscriminatorFact {
        value,
        reason: python_missing_discriminator_reason(probe_family, line),
        flow_sink: flow_sink.cloned(),
    }]
}

fn python_missing_discriminator_value(
    probe_family: &ProbeFamily,
    line_text: &str,
) -> Option<String> {
    match probe_family {
        ProbeFamily::Predicate => python_boundary_discriminator(line_text),
        ProbeFamily::ReturnValue => python_return_value_discriminator(line_text),
        ProbeFamily::ErrorPath => python_exception_discriminator(line_text),
        ProbeFamily::FieldConstruction => python_field_value_discriminator(line_text),
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
            python_output_or_call_discriminator(line_text)
        }
        ProbeFamily::MatchArm | ProbeFamily::StaticUnknown => None,
    }
}

fn python_missing_discriminator_reason(probe_family: &ProbeFamily, line: usize) -> String {
    let shape = match probe_family {
        ProbeFamily::Predicate => "equality-boundary",
        ProbeFamily::ReturnValue => "returned-value",
        ProbeFamily::ErrorPath => "exception",
        ProbeFamily::FieldConstruction => "field/object value",
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => "output/log/call effect",
        ProbeFamily::MatchArm => "match-arm",
        ProbeFamily::StaticUnknown => "static",
    };
    format!("changed Python {shape} at line {line} lacks a concrete repair discriminator")
}

fn python_boundary_discriminator(line_text: &str) -> Option<String> {
    let expression = strip_python_control_prefix(line_text);
    for operator in [">=", "<=", ">", "<"] {
        if let Some(idx) = expression.find(operator) {
            let left = comparison_operand_before(&expression, idx)?;
            let right = comparison_operand_after(&expression, idx + operator.len())?;
            if is_simple_python_discriminator_operand(&left)
                && is_simple_python_discriminator_operand(&right)
            {
                return Some(format!("{left} == {right}"));
            }
        }
    }
    None
}

fn python_return_value_discriminator(line_text: &str) -> Option<String> {
    let expression = line_text.trim().strip_prefix("return ")?.trim();
    if expression.is_empty() {
        None
    } else {
        Some(format!("return value == {expression}"))
    }
}

fn python_exception_discriminator(line_text: &str) -> Option<String> {
    let raised = line_text.trim().strip_prefix("raise ")?.trim();
    if raised.is_empty() {
        return None;
    }
    let exception_type = raised
        .split_once('(')
        .map(|(ty, _)| ty.trim())
        .unwrap_or(raised)
        .trim();
    if exception_type.is_empty() {
        return None;
    }
    if let Some(message) = first_python_string_literal(raised) {
        Some(format!("raises {exception_type} matching {message}"))
    } else {
        Some(format!("raises {exception_type}"))
    }
}

fn python_field_value_discriminator(line_text: &str) -> Option<String> {
    let text = line_text.trim();
    if let Some(discriminator) = python_return_dict_field_discriminator(text) {
        return Some(discriminator);
    }
    let (lhs, rhs) = split_python_assignment(text)?;
    if lhs.is_empty() || rhs.is_empty() {
        return None;
    }
    Some(format!("{lhs} == {rhs}"))
}

fn python_return_dict_field_discriminator(line_text: &str) -> Option<String> {
    let expression = line_text.trim().strip_prefix("return ")?.trim();
    let body = expression
        .strip_prefix('{')?
        .trim_start()
        .trim_end_matches('}')
        .trim_end();
    let (raw_key, rest) = body.split_once(':')?;
    let key = raw_key.trim().trim_matches('"').trim_matches('\'');
    let value = rest
        .split(',')
        .next()
        .unwrap_or(rest)
        .trim()
        .trim_end_matches('}')
        .trim();
    if key.is_empty() || value.is_empty() {
        None
    } else {
        Some(format!("{key} == {value}"))
    }
}

fn python_output_or_call_discriminator(line_text: &str) -> Option<String> {
    let text = line_text.trim();
    let literal = first_python_string_literal(text)?;
    if text.starts_with("print(") {
        Some(format!("output contains {literal}"))
    } else if text.contains("logger.") || text.contains("logging.") {
        Some(format!("log contains {literal}"))
    } else {
        Some(format!("call includes {literal}"))
    }
}

fn split_python_assignment(text: &str) -> Option<(&str, &str)> {
    if text.contains("==") || text.contains("!=") || text.contains(">=") || text.contains("<=") {
        return None;
    }
    let (lhs, rhs) = text.split_once('=')?;
    Some((lhs.trim(), rhs.trim()))
}

fn first_python_string_literal(text: &str) -> Option<String> {
    let mut start = None;
    let mut escaped = false;
    for (idx, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' || ch == '\'' {
            start = Some((idx, ch));
            break;
        }
    }
    let (start_idx, quote) = start?;
    escaped = false;
    for (relative_idx, ch) in text[start_idx + quote.len_utf8()..].char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            let end_idx = start_idx + quote.len_utf8() + relative_idx + quote.len_utf8();
            return text.get(start_idx..end_idx).map(str::to_string);
        }
    }
    None
}

fn strip_python_control_prefix(line_text: &str) -> String {
    let mut text = line_text.trim().trim_end_matches(':').trim().to_string();
    for prefix in ["if ", "elif ", "while ", "case "] {
        if let Some(stripped) = text.strip_prefix(prefix) {
            text = stripped.trim().to_string();
            break;
        }
    }
    text
}

fn is_python_control_predicate_line(line_text: &str) -> bool {
    let trimmed = line_text.trim_start();
    (trimmed.contains(" if ") && trimmed.contains(" else "))
        || trimmed.starts_with("if ")
        || trimmed.starts_with("elif ")
        || trimmed.starts_with("while ")
        || trimmed.starts_with("for ")
        || trimmed.starts_with("match ")
        || trimmed.starts_with("case ")
}

fn comparison_operand_before(expression: &str, operator_start: usize) -> Option<String> {
    let left = expression.get(..operator_start)?.trim_end();
    let operand = left
        .rsplit(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '(' | ')' | '[' | ']' | '{' | '}' | ',' | ':' | '+' | '-' | '*' | '/' | '%'
                )
        })
        .find(|part| !part.is_empty())?;
    Some(operand.trim().to_string())
}

fn comparison_operand_after(expression: &str, operator_end: usize) -> Option<String> {
    let right = expression.get(operator_end..)?.trim_start();
    let operand = right
        .split(|ch: char| {
            ch.is_whitespace()
                || matches!(
                    ch,
                    '(' | ')' | '[' | ']' | '{' | '}' | ',' | ':' | '+' | '-' | '*' | '/' | '%'
                )
        })
        .find(|part| !part.is_empty())?;
    Some(operand.trim().to_string())
}

fn is_simple_python_discriminator_operand(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|ch| {
            ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '"' || ch == '\''
        })
}

fn stop_reason_for_python_static_limit(limit: &PythonStaticLimit) -> StopReason {
    match limit.kind {
        StaticLimitKind::DynamicDispatch => StopReason::DynamicDispatchUnresolved,
        _ => StopReason::StaticProbeUnknown,
    }
}

fn python_weak_missing_summary(
    owner: &PythonOwner,
    probe_family: &ProbeFamily,
    strongest_kind: &OracleKind,
) -> String {
    let shape = match probe_family {
        ProbeFamily::Predicate => "the changed boundary",
        ProbeFamily::ReturnValue => "the returned value",
        ProbeFamily::ErrorPath => "the exact exception type/message",
        ProbeFamily::FieldConstruction => "the changed field or object value",
        ProbeFamily::SideEffect | ProbeFamily::CallDeletion => "the changed output/log/call effect",
        ProbeFamily::MatchArm => "the changed match arm",
        ProbeFamily::StaticUnknown => "the changed behavior",
    };
    format!(
        "Related Python test reaches `{}` but the strongest extracted oracle is `{}`; add or strengthen a focused assertion for {shape}.",
        owner.name,
        strongest_kind.as_str()
    )
}

fn python_recommended_next_step(
    class: &ExposureClass,
    probe_family: &ProbeFamily,
    has_oracle_eligible_relation: bool,
    missing_discriminators: &[MissingDiscriminatorFact],
) -> Option<String> {
    match class {
        ExposureClass::StaticUnknown | ExposureClass::NoStaticPath => None,
        ExposureClass::Exposed => {
            Some("Python preview: changed behavior is observed under a strong oracle; verify the assertion targets the changed behavior.".to_string())
        }
        _ if !has_oracle_eligible_relation => None,
        _ => {
            let missing = &missing_discriminators.first()?.value;
            let action = match probe_family {
                ProbeFamily::Predicate => "strengthen the existing related test with a focused boundary assertion",
                ProbeFamily::ReturnValue => {
                    "strengthen the existing related test with an exact return-value assertion"
                }
                ProbeFamily::ErrorPath => {
                    "strengthen the existing related test with an exception assertion"
                }
                ProbeFamily::FieldConstruction => {
                    "strengthen the existing related test with a field/object assertion"
                }
                ProbeFamily::SideEffect | ProbeFamily::CallDeletion => {
                    "strengthen the existing related test with an output/log/call-effect assertion"
                }
                ProbeFamily::MatchArm | ProbeFamily::StaticUnknown => {
                    "strengthen the existing related test with a focused assertion"
                }
            };
            Some(format!(
                "Python preview: {action} for missing discriminator `{missing}`."
            ))
        }
    }
}

fn classify_change(
    file: &Path,
    line: usize,
    line_text: &str,
    owners: &[PythonOwner],
    all_tests: &[PythonTest],
) -> Option<Finding> {
    let owner = owner_for_changed_line(file, line, owners)?;
    let related_candidates = related_test_candidates(owner, all_tests);
    let related = find_related_tests(owner, all_tests);
    let static_limit = static_limit_for_change(line_text, owner, &related_candidates);
    let (family, delta) = classify_probe_shape(line_text);
    let has_oracle_eligible_relation = related_candidates
        .iter()
        .any(|candidate| candidate.relation.uses_oracle());
    let strongest_strength = related
        .iter()
        .map(|test| test.oracle_strength.rank())
        .max()
        .unwrap_or(0);
    let strongest_kind = related
        .iter()
        .max_by_key(|test| test.oracle_strength.rank())
        .map(|test| test.oracle_kind.clone())
        .unwrap_or(OracleKind::Unknown);

    let (class, reach_state, observe_state, discriminate_state, mut missing) = if static_limit
        .is_some()
    {
        (
            ExposureClass::StaticUnknown,
            if related.is_empty() {
                StageState::No
            } else if has_oracle_eligible_relation {
                StageState::Yes
            } else {
                StageState::Weak
            },
            if related.is_empty() {
                StageState::No
            } else if has_oracle_eligible_relation {
                StageState::Yes
            } else {
                StageState::Weak
            },
            if related.is_empty() {
                StageState::No
            } else if strongest_strength >= OracleStrength::Strong.rank() {
                StageState::Yes
            } else {
                StageState::Weak
            },
            Vec::new(),
        )
    } else if related.is_empty() {
        (
            ExposureClass::NoStaticPath,
            StageState::No,
            StageState::No,
            StageState::No,
            vec![format!(
                "No Python test references {}; add a pytest or unittest test that calls the changed owner.",
                owner.missing_test_reference()
            )],
        )
    } else if !has_oracle_eligible_relation {
        (
            ExposureClass::WeaklyExposed,
            StageState::Weak,
            StageState::Weak,
            StageState::Weak,
            vec![format!(
                "Only heuristic Python test links were found for `{}`; verify the suggested test location or add a direct pytest or unittest call with an exact-value assertion.",
                owner.name
            )],
        )
    } else if strongest_strength >= OracleStrength::Strong.rank() {
        (
            ExposureClass::Exposed,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            vec![format!(
                "Related Python test reaches `{}` with a `{}` oracle. Static evidence suggests the changed behavior is observed under an exact-value discriminator.",
                owner.name,
                strongest_kind.as_str()
            )],
        )
    } else {
        (
            ExposureClass::WeaklyExposed,
            StageState::Yes,
            StageState::Weak,
            StageState::Weak,
            vec![python_weak_missing_summary(owner, &family, &strongest_kind)],
        )
    };
    if let Some(limit) = &static_limit {
        missing.push(limit.missing.clone());
    }

    let id_path: String = file
        .display()
        .to_string()
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let canonical_gap = static_limit
        .is_none()
        .then(|| canonical_python_gap_for(file, owner, &family, line_text));
    let probe = Probe {
        id: ProbeId(format!("probe:{id_path}:{line}:python_preview")),
        location: SourceLocation::new(file.to_string_lossy().as_ref(), line, 1),
        owner: Some(owner.symbol_id()),
        family: family.clone(),
        delta,
        before: None,
        after: Some(line_text.to_string()),
        expression: line_text.to_string(),
        expected_sinks: Vec::new(),
        required_oracles: Vec::new(),
    };

    let related_count = related.len();
    let reach_summary = if related_count == 0 {
        format!("0 related Python test(s) found for owner `{}`", owner.name)
    } else if has_oracle_eligible_relation {
        format!(
            "{} related Python test(s) found for owner `{}`",
            related_count, owner.name
        )
    } else {
        format!(
            "{} heuristic Python test link(s) found for owner `{}`; relation is uncertain",
            related_count, owner.name
        )
    };
    let reach = StageEvidence::new(reach_state, Confidence::Low, &reach_summary);
    let infect = StageEvidence::new(
        if static_limit.is_some() {
            StageState::Unknown
        } else {
            StageState::Yes
        },
        Confidence::Low,
        if let Some(limit) = &static_limit {
            format!(
                "Static limit `{}` prevents a safe Python infection claim.",
                limit.kind.as_str()
            )
        } else {
            python_infection_evidence(&family, line_text).summary
        },
    );
    let propagate = python_propagation_evidence(&family, line_text, static_limit.as_ref());
    let flow_sink = static_limit
        .is_none()
        .then(|| python_flow_sink_for(&family, owner, line, line_text))
        .flatten();
    let missing_discriminators = if static_limit.is_none()
        && matches!(class, ExposureClass::WeaklyExposed)
        && has_oracle_eligible_relation
    {
        python_missing_discriminators(&family, line, line_text, flow_sink.as_ref())
    } else {
        Vec::new()
    };
    let observe = StageEvidence::new(
        observe_state,
        Confidence::Low,
        format!(
            "Strongest extracted Python oracle kind: `{}` (rank {})",
            strongest_kind.as_str(),
            strongest_strength
        ),
    );
    let discriminate_summary = if strongest_strength >= OracleStrength::Strong.rank() {
        format!(
            "Related Python test uses a `{}` oracle; static evidence suggests the changed behavior is discriminated.",
            strongest_kind.as_str()
        )
    } else {
        missing_discriminators
            .first()
            .map(|missing| {
                format!(
                    "Python preview adapter found no strong discriminator; missing proof: `{}`.",
                    missing.value
                )
            })
            .unwrap_or_else(|| {
                "Python preview adapter found no strong discriminator; typed repair guidance is unavailable for this shape.".to_string()
            })
    };
    let discriminate =
        StageEvidence::new(discriminate_state, Confidence::Low, discriminate_summary);

    let recommended = python_recommended_next_step(
        &class,
        &family,
        has_oracle_eligible_relation,
        &missing_discriminators,
    );
    let repair_placement = python_repair_placement(&class, &related_candidates);
    let confidence_value = if matches!(class, ExposureClass::Exposed) {
        0.6
    } else if matches!(class, ExposureClass::StaticUnknown) {
        0.2
    } else {
        0.4
    };

    let mut evidence = vec![
        format!("owner: {}", owner.qualified_name),
        format!("owner_kind: {}", owner.kind_label()),
    ];
    if !owner.decorators.is_empty() {
        evidence.push(format!("owner_decorators: {}", owner.decorators.join(", ")));
    }
    if let Some(limit) = &static_limit {
        evidence.push(limit.evidence.clone());
    }
    for discriminator in &missing_discriminators {
        evidence.push(format!("missing_discriminator: {}", discriminator.value));
    }
    if let Some(placement) = &repair_placement {
        evidence.push(format!(
            "suggested_repair_action: {}",
            placement.repair_action
        ));
        evidence.push(format!(
            "suggested_test_file: {}",
            placement.suggested_test_file
        ));
        evidence.push(format!(
            "suggested_test_name: {}",
            placement.suggested_test_name
        ));
        if let Some(node_id) = &placement.suggested_test_node_id {
            evidence.push(format!("suggested_test_node_id: {node_id}"));
        }
        evidence.push(format!(
            "suggested_verify_command: {}",
            placement.verify_command
        ));
        evidence.push(format!(
            "suggested_verify_command_confidence: {}",
            placement.verify_command_confidence
        ));
        evidence.push(format!(
            "suggested_test_location_reason: {}",
            placement.location_reason
        ));
    }
    for candidate in related_candidates {
        let test = candidate.test;
        evidence.push(format!(
            "test_framework: {} ({})",
            test.framework, test.name
        ));
        if !test.fixtures.is_empty() {
            evidence.push(format!(
                "test_fixtures: {} ({})",
                test.fixtures.join(", "),
                test.name
            ));
        }
        if test.parametrized {
            evidence.push(format!("test_parametrized: pytest ({})", test.name));
        }
        if let Some(command) = verify_command_for_test(test) {
            evidence.push(format!("test_verify_command: {command} ({})", test.name));
        }
        evidence.push(format!(
            "related_test_relation: {} ({})",
            candidate.relation.as_str(),
            test.name
        ));
        if candidate.relation.is_uncertain() {
            evidence.push(format!(
                "related_test_uncertain: {} ({})",
                candidate.relation.as_str(),
                test.name
            ));
        }
        if candidate.relation.uses_oracle()
            && let Some(assertion) = strongest_assertion(&test.assertions)
        {
            evidence.push(format!(
                "test_oracle: {} {} ({})",
                assertion.oracle_kind.as_str(),
                assertion.oracle_strength.as_str(),
                test.name
            ));
            if assertion.oracle_shape != PythonOracleShape::ExactAssertion {
                evidence.push(format!(
                    "test_oracle_shape: {} ({})",
                    assertion.oracle_shape.as_str(),
                    test.name
                ));
            }
        } else if candidate.relation.uses_oracle() {
            evidence.push(format!("test_oracle_shape: reach_only ({})", test.name));
        }
    }

    Some(Finding {
        id: probe.id.0.clone(),
        canonical_gap,
        probe,
        class,
        ripr: RiprEvidence {
            reach,
            infect,
            propagate,
            reveal: RevealEvidence {
                observe,
                discriminate,
            },
        },
        confidence: confidence_value,
        evidence,
        missing,
        flow_sinks: flow_sink.into_iter().collect(),
        activation: crate::domain::ActivationEvidence {
            observed_values: Vec::new(),
            missing_discriminators,
        },
        stop_reasons: static_limit
            .as_ref()
            .map(stop_reason_for_python_static_limit)
            .into_iter()
            .collect(),
        related_tests: related,
        recommended_next_step: recommended,
        language: Some(DomainLanguageId::Python),
        language_status: Some(LanguageStatus::Preview),
        owner_kind: owner.owner_kind,
        static_limit_kind: static_limit.map(|limit| limit.kind),
    })
}

fn owner_for_changed_line<'a>(
    file: &Path,
    line: usize,
    owners: &'a [PythonOwner],
) -> Option<&'a PythonOwner> {
    let changed_file = normalized_path(file);
    owners
        .iter()
        .filter(|owner| normalized_path(&owner.file) == changed_file)
        .filter(|owner| line >= owner.start_line && line <= owner.end_line)
        .min_by_key(|owner| (owner.span_width(), owner.specificity_rank()))
}

fn collect_workspace_python_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    visit_workspace(root, root, &mut out);
    out.sort();
    out
}

fn visit_workspace(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        if is_python_workspace_excluded_dir(name) {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if file_type.is_dir() {
            visit_workspace(root, &path, out);
        } else if file_type.is_file() {
            let adapter = PythonAdapter;
            if adapter.accepts_path(&path) && !is_detectable_generated_python_file(&path) {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                out.push(relative);
            }
        }
    }
}

fn is_python_workspace_excluded_dir(name: &str) -> bool {
    PYTHON_WORKSPACE_EXCLUDED_DIRS.contains(&name)
}

fn is_detectable_generated_python_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.ends_with("_pb2.py")
        || name.ends_with("_pb2_grpc.py")
        || name.ends_with(".generated.py")
        || name.ends_with("_generated.py")
        || name.starts_with("generated_")
}

impl LanguageAdapter for PythonAdapter {
    fn accepts_path(&self, path: &Path) -> bool {
        matches!(route(path), Some(LanguageId::Python))
    }

    fn analyze_diff(
        &self,
        options: &AnalysisOptions,
        _oracle_policy: &OraclePolicy,
        changed_files: &[ChangedFile],
    ) -> Result<LanguageDiffResult, String> {
        let workspace_files = collect_workspace_python_files(&options.root);
        let mut all_owners: Vec<PythonOwner> = Vec::new();
        let mut all_tests: Vec<PythonTest> = Vec::new();
        for relative in &workspace_files {
            let absolute = options.root.join(relative);
            let Ok(source) = std::fs::read_to_string(&absolute) else {
                continue;
            };
            let facts = extract_source_facts(relative, &source);
            debug_assert!(source_fact_snapshot_observation(&facts) > 0);
            if is_test_file(relative) {
                all_tests.extend(facts.tests);
            } else {
                all_owners.extend(facts.owners);
            }
        }

        let mut findings: Vec<Finding> = Vec::new();
        let mut changed_count: usize = 0;
        for changed in changed_files {
            if !self.accepts_path(&changed.path) {
                continue;
            }
            changed_count += 1;
            if is_test_file(&changed.path) {
                continue;
            }
            for added in &changed.added_lines {
                if let Some(finding) = classify_change(
                    &changed.path,
                    added.line,
                    &added.text,
                    &all_owners,
                    &all_tests,
                ) {
                    findings.push(finding);
                }
            }
        }
        Ok(LanguageDiffResult {
            findings,
            changed_files: changed_count,
        })
    }

    fn analyze_repo(
        &self,
        _options: &AnalysisOptions,
        _oracle_policy: &OraclePolicy,
    ) -> Result<LanguageRepoResult, String> {
        // Repo-mode preview output lands in a follow-up. The current
        // sub-slice scopes to diff-mode for the smallest useful fixture.
        Ok(LanguageRepoResult {
            findings: Vec::new(),
            production_files: 0,
        })
    }
}

#[cfg(test)]
mod python_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn changed(path: &str) -> ChangedFile {
        ChangedFile {
            path: PathBuf::from(path),
            added_lines: Vec::new(),
            removed_lines: Vec::new(),
        }
    }

    fn missing_discriminator_values(finding: &Finding) -> Vec<&str> {
        finding
            .activation
            .missing_discriminators
            .iter()
            .map(|missing| missing.value.as_str())
            .collect()
    }

    fn evidence_value<'a>(finding: &'a Finding, prefix: &str) -> Option<&'a str> {
        finding
            .evidence
            .iter()
            .find_map(|entry| entry.strip_prefix(prefix))
    }

    #[test]
    fn accepts_py_paths() {
        let adapter = PythonAdapter;
        assert!(adapter.accepts_path(Path::new("scripts/run.py")));
        assert!(adapter.accepts_path(Path::new("src/lib/util.py")));
        assert!(!adapter.accepts_path(Path::new("src/lib.rs")));
        assert!(!adapter.accepts_path(Path::new("src/index.ts")));
        assert!(!adapter.accepts_path(Path::new("src/index.tsx")));
        assert!(!adapter.accepts_path(Path::new("README.md")));
        assert!(!adapter.accepts_path(Path::new("no-extension")));
    }

    #[test]
    fn parse_source_accepts_simple_python() {
        let ok = parse_module(
            Path::new("src/discount.py"),
            "def discount(amount: int) -> int:\n    return amount\n",
        )
        .is_some();
        assert!(ok, "valid Python should parse without errors");
    }

    #[test]
    fn parse_source_accepts_class_and_decorator() {
        let ok = parse_module(
            Path::new("src/repo.py"),
            "class Repo:\n    @staticmethod\n    def make() -> 'Repo':\n        return Repo()\n",
        )
        .is_some();
        assert!(ok, "decorated class methods should parse");
    }

    #[test]
    fn parse_source_accepts_async_def_and_fstring() {
        let ok = parse_module(
            Path::new("src/http.py"),
            "async def load(url: str) -> str:\n    return f\"{url}!\"\n",
        )
        .is_some();
        assert!(ok, "async def + f-string should parse");
    }

    #[test]
    fn parse_source_rejects_garbage() {
        let ok = parse_module(
            Path::new("src/oops.py"),
            "this is not :: valid +++ python at all",
        )
        .is_some();
        assert!(!ok, "garbage source should produce parse errors");
    }

    #[test]
    fn extract_owners_recognizes_functions_and_methods() {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            r#"
def apply_discount(amount):
    return amount

async def load_total(client):
    return await client.total()

class Policy:
    def apply(self, amount):
        return amount

    @staticmethod
    def normalize(amount):
        return amount

    @classmethod
    def from_config(cls, config):
        return cls()
"#,
        );

        assert_eq!(
            owners
                .iter()
                .map(|owner| owner.qualified_name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "apply_discount",
                "load_total",
                "Policy.apply",
                "Policy.normalize",
                "Policy.from_config",
                "Policy",
                "<module>"
            ]
        );
        assert_eq!(owners[0].owner_kind, Some(OwnerKind::Function));
        assert_eq!(owners[1].decorators, vec!["async_def"]);
        assert_eq!(owners[2].owner_kind, Some(OwnerKind::Method));
        assert_eq!(owners[3].owner_kind, Some(OwnerKind::ClassMethod));
        assert_eq!(owners[4].owner_kind, Some(OwnerKind::ClassMethod));
    }

    #[test]
    fn extract_tests_recognizes_pytest_parametrize_and_unittest() {
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            r#"
import unittest
import pytest

@pytest.mark.parametrize("amount", [1, 2])
def test_apply_discount(amount):
    apply_discount(amount)

class TestPytestStyle:
    def test_class_style(self, client):
        assert client.get("/discount").status_code == 200

class Helper:
    def test_not_a_pytest_class(self):
        apply_discount(10)

class PriceTests(unittest.TestCase):
    def test_apply_method(self):
        Policy().apply(10)
"#,
        );

        assert_eq!(
            tests
                .iter()
                .map(|test| test.name.as_str())
                .collect::<Vec<_>>(),
            vec![
                "test_apply_discount",
                "test_class_style",
                "test_apply_method"
            ]
        );
        assert!(tests[0].parametrized);
        assert_eq!(tests[0].fixtures, vec!["amount".to_string()]);
        assert_eq!(tests[0].qualified_name, "test_apply_discount");
        assert_eq!(tests[0].framework, "pytest");
        assert_eq!(tests[1].fixtures, vec!["client".to_string()]);
        assert_eq!(tests[1].qualified_name, "TestPytestStyle.test_class_style");
        assert_eq!(tests[1].framework, "pytest");
        assert_eq!(tests[2].qualified_name, "PriceTests.test_apply_method");
        assert_eq!(tests[2].framework, "unittest");
        assert!(
            tests
                .iter()
                .all(|test| test.name != "test_not_a_pytest_class")
        );
    }

    #[test]
    fn extract_tests_records_module_import_aliases() {
        let tests = extract_tests(
            Path::new("tests/test_imports.py"),
            r#"
import src.catalog as catalog
from src.tax import apply_fee, apply_tax as taxed

def test_imports():
    assert catalog.calculate_total(10) == 17
    assert taxed(10) == 12
"#,
        );

        assert_eq!(
            tests[0]
                .imports
                .iter()
                .map(|import| (import.imported.as_str(), import.alias.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("src.catalog", "catalog"),
                ("apply_fee", "apply_fee"),
                ("apply_tax", "taxed")
            ]
        );
    }

    #[test]
    fn extract_tests_collects_pytest_assertion_oracles() {
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            r#"
def test_apply_discount_exact():
    assert apply_discount(100, 50) == 90

def test_apply_discount_negative():
    assert apply_discount(10, 50) != 90

def test_apply_discount_smoke():
    assert apply_discount(10, 50)

def test_apply_discount_type():
    assert isinstance(apply_discount(10, 50), int)
"#,
        );

        assert_eq!(tests.len(), 4);
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(
            tests[0].assertions[0].oracle_strength,
            OracleStrength::Strong
        );
        assert_eq!(
            tests[1].assertions[0].oracle_kind,
            OracleKind::RelationalCheck
        );
        assert_eq!(tests[1].assertions[0].oracle_strength, OracleStrength::Weak);
        assert_eq!(tests[2].assertions[0].oracle_kind, OracleKind::SmokeOnly);
        assert_eq!(
            tests[2].assertions[0].oracle_strength,
            OracleStrength::Smoke
        );
        assert_eq!(
            tests[3].assertions[0].oracle_kind,
            OracleKind::RelationalCheck
        );
        assert_eq!(tests[3].assertions[0].oracle_strength, OracleStrength::Weak);
    }

    #[test]
    fn extract_tests_collects_pytest_raises_oracle() {
        let tests = extract_tests(
            Path::new("tests/test_validation.py"),
            r#"
import pytest

def test_apply_discount_rejects_negative():
    with pytest.raises(ValueError):
        apply_discount(-1, 50)
"#,
        );

        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::BroadError);
        assert_eq!(tests[0].assertions[0].oracle_strength, OracleStrength::Weak);
    }

    #[test]
    fn extract_tests_collects_unittest_assertion_oracles() {
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            r#"
import unittest

class PriceTests(unittest.TestCase):
    def test_apply_discount_exact(self):
        self.assertEqual(apply_discount(100, 50), 90)

    def test_apply_discount_raises(self):
        with self.assertRaises(ValueError):
            apply_discount(-1, 50)

    def test_apply_discount_boolean(self):
        self.assertTrue(apply_discount(10, 50) >= 0)
"#,
        );

        assert_eq!(tests.len(), 3);
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(
            tests[0].assertions[0].oracle_strength,
            OracleStrength::Strong
        );
        assert_eq!(tests[1].assertions[0].oracle_kind, OracleKind::BroadError);
        assert_eq!(tests[1].assertions[0].oracle_strength, OracleStrength::Weak);
        assert_eq!(tests[2].assertions[0].oracle_kind, OracleKind::SmokeOnly);
        assert_eq!(
            tests[2].assertions[0].oracle_strength,
            OracleStrength::Smoke
        );
    }

    #[test]
    fn extract_tests_collects_mock_call_oracle() {
        let tests = extract_tests(
            Path::new("tests/test_notifier.py"),
            r#"
def test_notifies_callback():
    callback = Mock()
    send_alert(callback)
    callback.assert_called_once_with("sent")
"#,
        );

        assert_eq!(tests.len(), 1);
        assert_eq!(
            tests[0].assertions[0].oracle_kind,
            OracleKind::MockExpectation
        );
        assert_eq!(
            tests[0].assertions[0].oracle_strength,
            OracleStrength::Medium
        );
    }

    #[test]
    fn classify_probe_shape_recognizes_python_predicate_shapes() {
        let (family, delta) = classify_probe_shape("    if amount >= threshold:");
        assert_eq!(family, ProbeFamily::Predicate);
        assert_eq!(delta, DeltaKind::Control);

        let (family, delta) =
            classify_probe_shape("    label = \"high\" if amount >= threshold else \"normal\"");
        assert_eq!(family, ProbeFamily::Predicate);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognizes_python_return_and_error_shapes() {
        let (family, delta) = classify_probe_shape("    return amount - 10");
        assert_eq!(family, ProbeFamily::ReturnValue);
        assert_eq!(delta, DeltaKind::Value);

        let (family, delta) = classify_probe_shape("    raise ValueError(\"bad\")");
        assert_eq!(family, ProbeFamily::ErrorPath);
        assert_eq!(delta, DeltaKind::Control);

        let (family, delta) = classify_probe_shape("    except ValueError:");
        assert_eq!(family, ProbeFamily::ErrorPath);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognizes_python_field_and_call_shapes() {
        let (family, delta) = classify_probe_shape("    self.status = \"paid\"");
        assert_eq!(family, ProbeFamily::FieldConstruction);
        assert_eq!(delta, DeltaKind::Value);

        let (family, delta) = classify_probe_shape("    notifier(\"receipt.sent\", order_id)");
        assert_eq!(family, ProbeFamily::SideEffect);
        assert_eq!(delta, DeltaKind::Effect);

        let (family, delta) = classify_probe_shape("    callback = MagicMock(name=\"receipt\")");
        assert_eq!(family, ProbeFamily::SideEffect);
        assert_eq!(delta, DeltaKind::Effect);
    }

    #[test]
    fn classify_change_returns_exposed_when_related_test_has_strong_oracle() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    if amount >= 100:\n        return amount - 10\n    return amount\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            "def test_apply_discount():\n    assert apply_discount(100) == 90\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    if amount >= 100:",
            &owners,
            &tests,
        ) else {
            return Err("changed line inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::Exposed);
        assert!(
            (finding.confidence - 0.6).abs() < 0.0001,
            "exposed Python preview confidence should be 0.6"
        );
        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Strong
        );
        assert!(finding.activation.missing_discriminators.is_empty());
        assert!(
            finding
                .evidence
                .iter()
                .all(|entry| !entry.starts_with("missing_discriminator:"))
        );
        Ok(())
    }

    #[test]
    fn classify_change_exposed_boundary_does_not_emit_missing_discriminator() -> Result<(), String>
    {
        let owners = extract_owners(
            Path::new("src/discount.py"),
            "def apply_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_discount.py"),
            "from src.discount import apply_discount\n\ndef test_apply_discount_boundary():\n    assert apply_discount(100, 100) == 90\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/discount.py"),
            2,
            "    if amount >= threshold:",
            &owners,
            &tests,
        ) else {
            return Err("changed predicate inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::Exposed);
        assert!(finding.activation.missing_discriminators.is_empty());
        assert!(
            finding
                .evidence
                .iter()
                .all(|entry| !entry.starts_with("missing_discriminator:"))
        );
        Ok(())
    }

    #[test]
    fn classify_change_returns_weakly_exposed_when_related_test_exists() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            "def test_apply_discount():\n    result = apply_discount(100, 50)\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    if amount >= threshold:",
            &owners,
            &tests,
        ) else {
            return Err("changed line inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(finding.language, Some(DomainLanguageId::Python));
        assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
        assert_eq!(finding.owner_kind, Some(OwnerKind::Function));
        assert_eq!(finding.ripr.reach.state, StageState::Yes);
        assert_eq!(finding.ripr.infect.state, StageState::Yes);
        assert_eq!(finding.ripr.propagate.state, StageState::Weak);
        assert_eq!(finding.ripr.reveal.observe.state, StageState::Weak);
        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Weak);
        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(finding.flow_sinks.len(), 1);
        assert_eq!(finding.flow_sinks[0].kind, FlowSinkKind::Unknown);
        assert_eq!(finding.activation.missing_discriminators.len(), 1);
        assert_eq!(
            finding.activation.missing_discriminators[0].value,
            "amount == threshold"
        );
        assert!(
            finding
                .evidence
                .iter()
                .any(|entry| entry == "missing_discriminator: amount == threshold")
        );
        assert!(finding.canonical_gap.is_some());
        assert!(finding.recommended_next_step.is_some());
        Ok(())
    }

    #[test]
    fn classify_change_emits_first_python_repair_class_discriminators() -> Result<(), String> {
        let return_finding = classify_change(
            Path::new("src/priority.py"),
            2,
            "    return amount >= 100",
            &extract_owners(
                Path::new("src/priority.py"),
                "def is_priority(amount):\n    return amount >= 100\n",
            ),
            &extract_tests(
                Path::new("tests/test_priority.py"),
                "from src.priority import is_priority\n\n\
                 def test_priority_amount():\n    assert is_priority(150)\n",
            ),
        )
        .ok_or_else(|| "return-value change should classify".to_string())?;
        assert_eq!(return_finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(
            missing_discriminator_values(&return_finding),
            vec!["return value == amount >= 100"]
        );
        assert!(
            return_finding
                .recommended_next_step
                .as_deref()
                .is_some_and(|step| step.contains("return-value assertion"))
        );

        let exception_finding = classify_change(
            Path::new("src/validation.py"),
            3,
            "        raise ValueError(\"positive required\")",
            &extract_owners(
                Path::new("src/validation.py"),
                "def require_positive(value):\n    if value <= 0:\n        raise ValueError(\"positive required\")\n    return value\n",
            ),
            &extract_tests(
                Path::new("tests/test_validation.py"),
                "import pytest\nfrom src.validation import require_positive\n\n\
                 def test_rejects_zero_value():\n    with pytest.raises(ValueError):\n        require_positive(0)\n",
            ),
        )
        .ok_or_else(|| "exception-path change should classify".to_string())?;
        assert_eq!(exception_finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(
            missing_discriminator_values(&exception_finding),
            vec!["raises ValueError matching \"positive required\""]
        );
        assert!(
            exception_finding
                .recommended_next_step
                .as_deref()
                .is_some_and(|step| step.contains("exception assertion"))
        );

        let field_finding = classify_change(
            Path::new("src/invoice.py"),
            3,
            "        self.status = \"paid\"",
            &extract_owners(
                Path::new("src/invoice.py"),
                "class Invoice:\n    def mark_paid(self):\n        self.status = \"paid\"\n",
            ),
            &extract_tests(
                Path::new("tests/test_invoice.py"),
                "from src.invoice import Invoice\n\n\
                 def test_mark_paid_smoke():\n    invoice = Invoice()\n    invoice.mark_paid()\n    assert invoice\n",
            ),
        )
        .ok_or_else(|| "field-value change should classify".to_string())?;
        assert_eq!(field_finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(
            missing_discriminator_values(&field_finding),
            vec!["self.status == \"paid\""]
        );
        assert!(
            field_finding
                .recommended_next_step
                .as_deref()
                .is_some_and(|step| step.contains("field/object assertion"))
        );

        let output_finding = classify_change(
            Path::new("src/notifications.py"),
            5,
            "    logger.warning(\"coupon expired\")",
            &extract_owners(
                Path::new("src/notifications.py"),
                "import logging\n\nlogger = logging.getLogger(__name__)\n\ndef warn_coupon():\n    logger.warning(\"coupon expired\")\n",
            ),
            &extract_tests(
                Path::new("tests/test_notifications.py"),
                "from src.notifications import warn_coupon\n\n\
                 def test_warn_coupon_smoke(caplog):\n    warn_coupon()\n    assert caplog.text\n",
            ),
        )
        .ok_or_else(|| "output/log change should classify".to_string())?;
        assert_eq!(output_finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(
            missing_discriminator_values(&output_finding),
            vec!["log contains \"coupon expired\""]
        );
        assert!(
            output_finding
                .recommended_next_step
                .as_deref()
                .is_some_and(|step| step.contains("output/log/call-effect assertion"))
        );

        Ok(())
    }

    #[test]
    fn classify_change_emits_python_repair_placement_and_verify_command() -> Result<(), String> {
        let pytest_finding = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    if amount >= threshold:",
            &extract_owners(
                Path::new("src/pricing.py"),
                "def calculate_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
            ),
            &extract_tests(
                Path::new("tests/test_pricing.py"),
                "from src.pricing import calculate_discount\n\n\
                 def test_calculate_discount_smoke():\n    result = calculate_discount(150, 100)\n    assert result\n",
            ),
        )
        .ok_or_else(|| "pytest boundary change should classify".to_string())?;
        assert_eq!(pytest_finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(
            evidence_value(&pytest_finding, "suggested_repair_action: "),
            Some("strengthen_existing_test")
        );
        assert_eq!(
            evidence_value(&pytest_finding, "suggested_test_file: "),
            Some("tests/test_pricing.py")
        );
        assert_eq!(
            evidence_value(&pytest_finding, "suggested_test_name: "),
            Some("test_calculate_discount_smoke")
        );
        assert_eq!(
            evidence_value(&pytest_finding, "suggested_test_node_id: "),
            Some("tests/test_pricing.py::test_calculate_discount_smoke")
        );
        assert_eq!(
            evidence_value(&pytest_finding, "suggested_verify_command: "),
            Some("pytest tests/test_pricing.py::test_calculate_discount_smoke")
        );
        assert_eq!(
            evidence_value(&pytest_finding, "suggested_verify_command_confidence: "),
            Some("high")
        );

        let unittest_finding = classify_change(
            Path::new("src/validation.py"),
            3,
            "        raise ValueError(\"positive required\")",
            &extract_owners(
                Path::new("src/validation.py"),
                "def require_positive(value):\n    if value <= 0:\n        raise ValueError(\"positive required\")\n    return value\n",
            ),
            &extract_tests(
                Path::new("tests/test_validation.py"),
                "import unittest\nfrom src.validation import require_positive\n\n\
                 class TestValidation(unittest.TestCase):\n    def test_rejects_zero_value(self):\n        with self.assertRaises(ValueError):\n            require_positive(0)\n",
            ),
        )
        .ok_or_else(|| "unittest exception change should classify".to_string())?;
        assert_eq!(unittest_finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(
            evidence_value(&unittest_finding, "suggested_repair_action: "),
            Some("strengthen_existing_test")
        );
        assert_eq!(
            evidence_value(&unittest_finding, "suggested_test_file: "),
            Some("tests/test_validation.py")
        );
        assert_eq!(
            evidence_value(&unittest_finding, "suggested_test_name: "),
            Some("test_rejects_zero_value")
        );
        assert_eq!(
            evidence_value(&unittest_finding, "suggested_test_node_id: "),
            None
        );
        assert_eq!(
            evidence_value(&unittest_finding, "suggested_verify_command: "),
            Some("python -m unittest tests.test_validation.TestValidation.test_rejects_zero_value")
        );
        assert_eq!(
            evidence_value(&unittest_finding, "suggested_verify_command_confidence: "),
            Some("high")
        );

        Ok(())
    }

    #[test]
    fn classify_change_suppresses_repair_guidance_for_non_actionable_python_cases()
    -> Result<(), String> {
        let exposed = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    if amount >= threshold:",
            &extract_owners(
                Path::new("src/pricing.py"),
                "def apply_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
            ),
            &extract_tests(
                Path::new("tests/test_pricing.py"),
                "from src.pricing import apply_discount\n\n\
                 def test_apply_discount_boundary():\n    assert apply_discount(100, 100) == 90\n",
            ),
        )
        .ok_or_else(|| "strong predicate change should classify".to_string())?;
        assert_eq!(exposed.class, ExposureClass::Exposed);
        assert!(exposed.activation.missing_discriminators.is_empty());
        assert!(
            exposed
                .recommended_next_step
                .as_deref()
                .is_some_and(|step| step.contains("observed under a strong oracle"))
        );

        let static_unknown = classify_change(
            Path::new("src/service.py"),
            2,
            "    return getattr(client, name)()",
            &extract_owners(
                Path::new("src/service.py"),
                "def call_named(client, name):\n    return getattr(client, name)()\n",
            ),
            &extract_tests(
                Path::new("tests/test_service.py"),
                "from src.service import call_named\n\n\
                 def test_call_named_dispatches():\n    assert call_named(client, \"total\") == 10\n",
            ),
        )
        .ok_or_else(|| "dynamic dispatch change should classify".to_string())?;
        assert_eq!(static_unknown.class, ExposureClass::StaticUnknown);
        assert!(static_unknown.activation.missing_discriminators.is_empty());
        assert!(static_unknown.recommended_next_step.is_none());

        let no_static_path = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    return amount - 10",
            &extract_owners(
                Path::new("src/pricing.py"),
                "def apply_discount(amount):\n    return amount - 10\n",
            ),
            &extract_tests(
                Path::new("tests/test_other.py"),
                "def test_other():\n    other_behavior()\n",
            ),
        )
        .ok_or_else(|| "unrelated return change should classify".to_string())?;
        assert_eq!(no_static_path.class, ExposureClass::NoStaticPath);
        assert!(no_static_path.activation.missing_discriminators.is_empty());
        assert!(no_static_path.recommended_next_step.is_none());

        Ok(())
    }

    #[test]
    fn classify_change_populates_language_qualified_owner_ids() -> Result<(), String> {
        let function_owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    return amount - 10\n",
        );
        let function = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    return amount - 10",
            &function_owners,
            &[],
        )
        .ok_or_else(|| "function changed line should classify".to_string())?;
        assert_eq!(
            function.probe.owner.as_ref().map(ToString::to_string),
            Some("python:src/pricing.py::apply_discount".to_string())
        );

        let method_owners = extract_owners(
            Path::new("src/cart.py"),
            "class Cart:\n    def apply_discount(self, amount):\n        return amount - 10\n",
        );
        let method = classify_change(
            Path::new("src/cart.py"),
            3,
            "        return amount - 10",
            &method_owners,
            &[],
        )
        .ok_or_else(|| "method changed line should classify".to_string())?;
        assert_eq!(
            method.probe.owner.as_ref().map(ToString::to_string),
            Some("python:src/cart.py::Cart.apply_discount".to_string())
        );

        let class_owners = extract_owners(
            Path::new("src/models.py"),
            "class Invoice:\n    status = \"pending\"\n\n    def mark_paid(self):\n        return \"paid\"\n",
        );
        let class_body = classify_change(
            Path::new("src/models.py"),
            2,
            "    status = \"pending\"",
            &class_owners,
            &[],
        )
        .ok_or_else(|| "class body changed line should classify".to_string())?;
        assert_eq!(
            class_body.probe.owner.as_ref().map(ToString::to_string),
            Some("python:src/models.py::Invoice".to_string())
        );
        assert_eq!(class_body.owner_kind, None);
        assert!(
            class_body
                .evidence
                .iter()
                .any(|entry| entry == "owner_kind: class")
        );

        let module_owners = extract_owners(
            Path::new("src/settings.py"),
            "DISCOUNT_THRESHOLD = 100\n\ndef threshold():\n    return DISCOUNT_THRESHOLD\n",
        );
        let module = classify_change(
            Path::new("src/settings.py"),
            1,
            "DISCOUNT_THRESHOLD = 100",
            &module_owners,
            &[],
        )
        .ok_or_else(|| "module changed line should classify".to_string())?;
        assert_eq!(
            module.probe.owner.as_ref().map(ToString::to_string),
            Some("python:src/settings.py::<module>".to_string())
        );
        assert_eq!(module.owner_kind, Some(OwnerKind::ModuleFunction));
        Ok(())
    }

    #[test]
    fn python_owner_id_is_stable_when_owner_line_moves() -> Result<(), String> {
        let before = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    return amount - 10\n",
        );
        let after = extract_owners(
            Path::new("src/pricing.py"),
            "\n\n\ndef apply_discount(amount):\n    return amount - 10\n",
        );
        let before_finding = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    return amount - 10",
            &before,
            &[],
        )
        .ok_or_else(|| "before owner should classify".to_string())?;
        let after_finding = classify_change(
            Path::new("src/pricing.py"),
            5,
            "    return amount - 10",
            &after,
            &[],
        )
        .ok_or_else(|| "after owner should classify".to_string())?;

        assert_eq!(before_finding.probe.owner, after_finding.probe.owner);
        assert_ne!(before_finding.probe.id, after_finding.probe.id);
        Ok(())
    }

    #[test]
    fn find_related_tests_matches_import_alias_call() {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    return amount - 10\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_alias_pricing.py"),
            "from src.pricing import apply_discount as discount\n\ndef test_discount_alias():\n    assert discount(100) == 90\n",
        );

        let related = find_related_tests(&owners[0], &tests);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "test_discount_alias");
        assert_eq!(related[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(related[0].oracle_strength, OracleStrength::Strong);
    }

    #[test]
    fn related_test_matching_ignores_object_method_calls_for_free_functions() {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    return amount - 10\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_order_methods.py"),
            "def test_order_discount_method():\n    assert order.apply_discount(100) == 90\n",
        );

        let related = related_test_candidates(&owners[0], &tests);

        assert!(related.is_empty());
    }

    #[test]
    fn related_test_matching_accepts_module_alias_attribute_calls() {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    return amount - 10\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_module_alias_pricing.py"),
            "import src.pricing as pricing\n\ndef test_discount_module_alias():\n    assert pricing.apply_discount(100) == 90\n",
        );

        let related = related_test_candidates(&owners[0], &tests);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].relation, PythonRelationKind::ImportAliasCall);
    }

    #[test]
    fn related_test_matching_keeps_method_owner_object_calls() {
        let owners = extract_owners(
            Path::new("src/cart.py"),
            "class Cart:\n    def apply_discount(self, amount):\n        return amount - 10\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_cart.py"),
            "def test_cart_discount_method():\n    assert cart.apply_discount(100) == 90\n",
        );

        let related = related_test_candidates(&owners[0], &tests);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].relation, PythonRelationKind::SyntacticCall);
    }

    #[test]
    fn classify_change_uses_import_alias_call_as_strong_relation() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/tax.py"),
            "def apply_tax(amount):\n    return amount + 2\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_checkout_tax.py"),
            "from src.tax import apply_tax as taxed\n\ndef test_checkout_tax_alias_import():\n    assert taxed(10) == 12\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/tax.py"),
            2,
            "    return amount + 2",
            &owners,
            &tests,
        ) else {
            return Err("changed line inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::Exposed);
        assert!(finding.evidence.iter().any(|entry| entry
            == "related_test_relation: import_alias_call (test_checkout_tax_alias_import)"));
        Ok(())
    }

    #[test]
    fn classify_change_uses_same_stem_test_as_weak_proximity() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    return amount - 10\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            "def test_boundary_documented_elsewhere():\n    assert 90 == 90\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    return amount - 10",
            &owners,
            &tests,
        ) else {
            return Err("changed line inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::Unknown);
        assert!(finding.evidence.iter().any(|entry| entry
            == "related_test_relation: same_stem (test_boundary_documented_elsewhere)"));
        Ok(())
    }

    #[test]
    fn same_stem_relation_accepts_suffix_and_orders_after_direct_calls() {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    return amount - 10\n",
        );
        let mut tests = extract_tests(
            Path::new("tests/pricing_test.py"),
            "def test_same_stem_only():\n    assert 90 == 90\n",
        );
        tests.extend(extract_tests(
            Path::new("tests/test_checkout.py"),
            "def test_direct_call():\n    assert apply_discount(100) == 90\n",
        ));

        let related = related_test_candidates(&owners[0], &tests);

        assert_eq!(normalize_test_stem("pricing_test"), "pricing");
        assert_eq!(related.len(), 2);
        assert_eq!(related[0].relation, PythonRelationKind::SyntacticCall);
        assert_eq!(related[1].relation, PythonRelationKind::SameStem);
    }

    #[test]
    fn static_limit_detection_covers_python_preview_limit_kinds() {
        let imported_owner = extract_owners(
            Path::new("src/service.py"),
            "from external.client import remote_total\n\ndef total():\n    return remote_total()\n",
        )
        .remove(0);
        let decorated_owner = extract_owners(
            Path::new("src/service.py"),
            "@retry(times=3)\ndef total():\n    return 1\n",
        )
        .remove(0);
        let plain_owner =
            extract_owners(Path::new("src/service.py"), "def total():\n    return 1\n").remove(0);
        let tests = extract_tests(
            Path::new("tests/test_service.py"),
            "from unittest.mock import patch\nfrom src.service import total\n\n@patch(\"src.service.remote_total\")\ndef test_total(mock_remote):\n    assert total() == 1\n",
        );
        let candidates = related_test_candidates(&plain_owner, &tests);
        let monkeypatch_tests = extract_tests(
            Path::new("tests/test_service.py"),
            "from src.service import total\n\ndef test_total(monkeypatch):\n    monkeypatch.setattr(\"src.service.remote_total\", lambda: 1)\n    assert total() == 1\n",
        );
        let monkeypatch_candidates = related_test_candidates(&plain_owner, &monkeypatch_tests);
        let opaque_helper_tests = extract_tests(
            Path::new("tests/test_service.py"),
            "from src.service import total\n\ndef test_total_custom_helper():\n    result = total()\n    assert_total_result(result)\n",
        );
        let opaque_helper_candidates = related_test_candidates(&plain_owner, &opaque_helper_tests);
        let opaque_helper_with_exact_tests = extract_tests(
            Path::new("tests/test_service.py"),
            "from src.service import total\n\ndef test_total_custom_helper_and_exact():\n    result = total()\n    assert_total_result(result)\n    assert result == 1\n",
        );
        let opaque_helper_with_exact_candidates =
            related_test_candidates(&plain_owner, &opaque_helper_with_exact_tests);

        assert_eq!(
            static_limit_for_change("    return getattr(client, name)()", &plain_owner, &[])
                .map(|limit| limit.kind),
            Some(StaticLimitKind::DynamicDispatch)
        );
        assert_eq!(
            static_limit_for_change("    return type(\"Dynamic\", (), {})", &plain_owner, &[])
                .map(|limit| limit.kind),
            Some(StaticLimitKind::Metaprogramming)
        );
        assert_eq!(
            static_limit_for_change("    return 1", &decorated_owner, &[]).map(|limit| limit.kind),
            Some(StaticLimitKind::DecoratorIndirection)
        );
        assert_eq!(
            static_limit_for_change("    return total()", &plain_owner, &candidates)
                .map(|limit| limit.kind),
            Some(StaticLimitKind::MockedModule)
        );
        assert_eq!(
            static_limit_for_change("    return total()", &plain_owner, &monkeypatch_candidates)
                .map(|limit| limit.kind),
            Some(StaticLimitKind::MockedModule)
        );
        assert_eq!(
            static_limit_for_change("    return 1", &plain_owner, &opaque_helper_candidates)
                .map(|limit| limit.kind),
            Some(StaticLimitKind::OpaqueCustomAssertionHelper)
        );
        assert_eq!(
            static_limit_for_change(
                "    return 1",
                &plain_owner,
                &opaque_helper_with_exact_candidates
            )
            .map(|limit| limit.kind),
            None
        );
        assert_eq!(
            static_limit_for_change("    return remote_total()", &imported_owner, &[])
                .map(|limit| limit.kind),
            Some(StaticLimitKind::MissingImportGraph)
        );
        assert_eq!(
            static_limit_for_change("    return lambda value: value + 1", &plain_owner, &[])
                .map(|limit| limit.kind),
            Some(StaticLimitKind::UnsupportedSyntax)
        );
        let mock_owner = extract_owners(
            Path::new("src/callbacks.py"),
            "from unittest.mock import MagicMock\n\ndef recording_callback():\n    callback = MagicMock(name=\"receipt\")\n    return callback\n",
        )
        .remove(0);
        assert_eq!(
            static_limit_for_change(
                "    callback = MagicMock(name=\"receipt.sent\")",
                &mock_owner,
                &[]
            )
            .map(|limit| limit.kind),
            None
        );
    }

    #[test]
    fn classify_change_static_limit_fails_closed_even_with_strong_oracle() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/service.py"),
            "def call_named(client, name):\n    return getattr(client, name)()\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_service.py"),
            "from src.service import call_named\n\ndef test_call_named_dispatches():\n    assert call_named(client, \"total\") == 10\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/service.py"),
            2,
            "    return getattr(client, name)()",
            &owners,
            &tests,
        ) else {
            return Err("changed line inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::StaticUnknown);
        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::DynamicDispatch)
        );
        assert_eq!(
            finding.stop_reasons,
            vec![StopReason::DynamicDispatchUnresolved]
        );
        assert!(finding.recommended_next_step.is_none());
        assert!(finding.canonical_gap.is_none());
        assert_eq!(finding.ripr.infect.state, StageState::Unknown);
        assert_eq!(finding.ripr.propagate.state, StageState::Unknown);
        assert!(
            finding
                .evidence
                .iter()
                .any(|entry| entry.starts_with("static_limit dynamic_dispatch:"))
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|entry| entry.contains("Static limit `dynamic_dispatch`"))
        );
        Ok(())
    }

    #[test]
    fn classify_change_opaque_custom_assertion_helper_fails_closed() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            "from src.pricing import apply_discount\n\ndef assert_discounted(result):\n    assert result < 100\n\ndef test_apply_discount_custom_helper():\n    result = apply_discount(100, 50)\n    assert_discounted(result)\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    if amount >= threshold:",
            &owners,
            &tests,
        ) else {
            return Err("changed predicate inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::StaticUnknown);
        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::OpaqueCustomAssertionHelper)
        );
        assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
        assert!(finding.canonical_gap.is_none());
        assert!(finding.recommended_next_step.is_none());
        assert!(finding.activation.missing_discriminators.is_empty());
        assert!(
            finding
                .evidence
                .iter()
                .any(|entry| entry.starts_with("static_limit opaque_custom_assertion_helper:"))
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|entry| entry.contains("Static limit `opaque_custom_assertion_helper`"))
        );
        Ok(())
    }

    #[test]
    fn classify_change_static_limit_omits_activation_discriminators() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/service.py"),
            "def has_named_value(client, name, threshold):\n    if getattr(client, name) >= threshold:\n        return True\n    return False\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_service.py"),
            "from src.service import has_named_value\n\ndef test_has_named_value():\n    assert has_named_value(client, \"total\", 10) is True\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/service.py"),
            2,
            "    if getattr(client, name) >= threshold:",
            &owners,
            &tests,
        ) else {
            return Err("changed predicate inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::StaticUnknown);
        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::DynamicDispatch)
        );
        assert!(finding.flow_sinks.is_empty());
        assert!(finding.activation.missing_discriminators.is_empty());
        assert!(
            finding
                .evidence
                .iter()
                .all(|entry| !entry.starts_with("missing_discriminator:"))
        );
        Ok(())
    }

    #[test]
    fn classify_change_returns_no_static_path_without_related_test() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    return amount - 10\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_other.py"),
            "def test_other():\n    other_behavior()\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    return amount - 10",
            &owners,
            &tests,
        ) else {
            return Err("changed line inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::NoStaticPath);
        assert_eq!(finding.owner_kind, Some(OwnerKind::Function));
        assert!(finding.related_tests.is_empty());
        assert_eq!(finding.ripr.reach.state, StageState::No);
        assert_eq!(finding.ripr.infect.state, StageState::Yes);
        assert_eq!(finding.ripr.propagate.state, StageState::Yes);
        assert!(finding.recommended_next_step.is_none());
        Ok(())
    }

    #[test]
    fn classify_change_ignores_unrelated_text_mentions() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    return amount - 10\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_docs.py"),
            "def test_docs_mentions_owner():\n    assert \"apply_discount(\" in \"apply_discount(\"\n",
        );

        let Some(finding) = classify_change(
            Path::new("src/pricing.py"),
            2,
            "    return amount - 10",
            &owners,
            &tests,
        ) else {
            return Err("changed line inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::NoStaticPath);
        assert!(finding.related_tests.is_empty());
        Ok(())
    }

    #[test]
    fn analyze_diff_returns_zero_findings_and_counts_accepted_files() -> Result<(), String> {
        let adapter = PythonAdapter;
        let options = AnalysisOptions {
            root: PathBuf::from("."),
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Draft,
            include_unchanged_tests: false,
        };
        let policy = OraclePolicy::default();
        let changed_files = vec![
            changed("scripts/run.py"),
            changed("src/lib.rs"),
            changed("docs/README.md"),
            changed("src/util.py"),
            changed("src/index.ts"),
        ];
        let result = adapter.analyze_diff(&options, &policy, &changed_files)?;
        assert!(result.findings.is_empty());
        assert_eq!(result.changed_files, 2);
        Ok(())
    }

    #[test]
    fn analyze_repo_returns_empty_scaffold() -> Result<(), String> {
        let adapter = PythonAdapter;
        let options = AnalysisOptions {
            root: PathBuf::from("."),
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Deep,
            include_unchanged_tests: false,
        };
        let policy = OraclePolicy::default();
        let result = adapter.analyze_repo(&options, &policy)?;
        assert!(result.findings.is_empty());
        assert_eq!(result.production_files, 0);
        Ok(())
    }
}
