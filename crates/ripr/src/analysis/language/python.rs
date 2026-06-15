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

use super::super::{
    AnalysisOptions, diff::ChangedFile, fingerprint_probe_id, normalize_expression,
};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::config::OraclePolicy;
use crate::domain::{
    Confidence, DeltaKind, ExposureClass, Finding, FindingCanonicalGap, FlowSinkFact, FlowSinkKind,
    LanguageId as DomainLanguageId, LanguageStatus, MissingDiscriminatorFact, OracleKind,
    OracleStrength, OwnerKind, Probe, ProbeFamily, RelatedTest, RevealEvidence, RiprEvidence,
    SourceLocation, StageEvidence, StageState, StaticLimitKind, StopReason, SymbolId,
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
    cli_receiver_names: Vec<String>,
    route_paths: Vec<String>,
    dynamic_route_decorators: Vec<String>,
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
    let route_paths = collect_static_route_paths(context.source, decorators);
    let dynamic_route_decorators = collect_dynamic_route_decorators(context.source, decorators);
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
        cli_receiver_names: collect_static_cli_receiver_names(context.source, context.imports),
        route_paths,
        dynamic_route_decorators,
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
        cli_receiver_names: collect_static_cli_receiver_names(context.source, context.imports),
        route_paths: collect_static_route_paths(context.source, decorators),
        dynamic_route_decorators: collect_dynamic_route_decorators(context.source, decorators),
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
        cli_receiver_names: collect_static_cli_receiver_names(source, imports),
        route_paths: Vec::new(),
        dynamic_route_decorators: Vec::new(),
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

fn collect_static_route_paths(source: &str, decorators: &[Expr]) -> Vec<String> {
    decorators
        .iter()
        .filter_map(|decorator| {
            let name = expr_full_name(decorator)?;
            if !is_static_route_decorator(&name) {
                return None;
            }
            route_decorator_literal_argument(source, decorator, &name)
        })
        .collect()
}

fn collect_dynamic_route_decorators(source: &str, decorators: &[Expr]) -> Vec<String> {
    decorators
        .iter()
        .filter_map(|decorator| {
            let name = expr_full_name(decorator)?;
            if !is_static_route_decorator(&name) {
                return None;
            }
            route_decorator_literal_argument(source, decorator, &name)
                .is_none()
                .then_some(name)
        })
        .collect()
}

fn route_decorator_literal_argument(source: &str, decorator: &Expr, name: &str) -> Option<String> {
    let text = text_for_range(source, decorator.range());
    let after_name = text
        .strip_prefix(name)
        .or_else(|| text.find(name).and_then(|idx| text.get(idx + name.len()..)))?;
    first_parenthesized_string_argument(after_name.trim_start())
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
        "assertRaisesRegex" => Some((
            OracleKind::ExactErrorVariant,
            OracleStrength::Strong,
            PythonOracleShape::ExceptionAssertion,
        )),
        "assertRaises" => Some((
            OracleKind::BroadError,
            OracleStrength::Weak,
            PythonOracleShape::ExceptionAssertion,
        )),
        "raises" if name == "pytest.raises" || name == "raises" => {
            if call_has_keyword(call, "match") {
                Some((
                    OracleKind::ExactErrorVariant,
                    OracleStrength::Strong,
                    PythonOracleShape::ExceptionAssertion,
                ))
            } else {
                Some((
                    OracleKind::BroadError,
                    OracleStrength::Weak,
                    PythonOracleShape::ExceptionAssertion,
                ))
            }
        }
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

fn call_has_keyword(call: &ast::ExprCall, name: &str) -> bool {
    call.keywords
        .iter()
        .any(|keyword| keyword.arg.as_ref().is_some_and(|arg| arg == name))
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
    ApiClientRouteCall,
    ConstructCall,
    LocalBinding,
    SameStem,
    TestNameSimilarity,
    FixtureName,
}

impl PythonRelationKind {
    fn rank(self) -> u8 {
        match self {
            Self::SyntacticCall => 5,
            Self::ImportAliasCall => 4,
            Self::ApiClientRouteCall => 4,
            Self::ConstructCall => 4,
            Self::LocalBinding => 4,
            Self::SameStem => 3,
            Self::TestNameSimilarity => 2,
            Self::FixtureName => 1,
        }
    }

    fn uses_oracle(self) -> bool {
        matches!(
            self,
            Self::SyntacticCall
                | Self::ImportAliasCall
                | Self::ApiClientRouteCall
                | Self::ConstructCall
                | Self::LocalBinding
        )
    }

    fn is_uncertain(self) -> bool {
        !self.uses_oracle()
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::SyntacticCall => "syntactic_call",
            Self::ImportAliasCall => "import_alias_call",
            Self::ApiClientRouteCall => "api_client_route_call",
            Self::ConstructCall => "construct_call",
            Self::LocalBinding => "local_binding",
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
                relation_reason: None,
                relation_confidence: None,
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
    if api_client_route_calls_owner(test, owner) {
        return Some(PythonRelationKind::ApiClientRouteCall);
    }
    if construct_call_invokes_owner(test, owner) {
        return Some(PythonRelationKind::ConstructCall);
    }
    if local_binding_calls_owner(test, owner) {
        return Some(PythonRelationKind::LocalBinding);
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

/// Detects an inline construct-call `OwnerClass(...)(...)` that invokes a changed
/// `__call__` owner directly (e.g. `LogfmtRenderer()(None, None, event_dict)`), a
/// cross-file shape the name/attribute and import-alias heuristics miss — the
/// changed sink is the class's `__call__`, but the test never names `__call__`.
/// Strictly gated so it never over-links: the changed owner must be a `__call__`
/// method (Guard A); the test must import the owner's class by name or alias
/// (Guard B), which blocks a same-named class from an unrelated module; and the
/// constructed instance must be *immediately* called (the balanced-paren check),
/// which distinguishes the inline `C()(...)` from a bound local `x = C(); x(...)`
/// (the latter stays uncertain, consistent with the local-callable limitation).
fn construct_call_invokes_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    // Guard A: only callable-class `__call__` owners.
    if owner.name != "__call__"
        || !matches!(
            owner.owner_kind,
            Some(OwnerKind::Method | OwnerKind::ClassMethod)
        )
    {
        return false;
    }
    let Some((class_name, _)) = owner.qualified_name.rsplit_once('.') else {
        return false;
    };
    if class_name.is_empty() || !class_name.chars().all(is_python_identifier_char) {
        return false;
    }
    // Guard B: the test must import the owner's class — blocks same-named classes
    // in unrelated modules from cross-linking.
    let imports_class = test
        .imports
        .iter()
        .any(|import| import.imported == class_name || import.alias == class_name);
    if !imports_class {
        return false;
    }
    let needle = format!("{class_name}(");
    test.body_text.match_indices(&needle).any(|(idx, _)| {
        has_call_boundary(&test.body_text, idx)
            && !line_prefix_looks_like_comment_or_string(&test.body_text, idx)
            && construct_result_is_called(&test.body_text, idx + needle.len() - 1)
    })
}

/// Given the byte index of the `(` that opens a constructor call, returns whether
/// its matching `)` is immediately followed (skipping spaces/tabs) by another `(`
/// — i.e. the constructed instance is called inline, `C(...)(...)`.
fn construct_result_is_called(text: &str, open_paren_idx: usize) -> bool {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut index = open_paren_idx;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    let mut next = index + 1;
                    while matches!(bytes.get(next), Some(b' ' | b'\t')) {
                        next += 1;
                    }
                    return bytes.get(next) == Some(&b'(');
                }
            }
            _ => {}
        }
        index += 1;
    }
    false
}

/// Detects a single unambiguous local binding `local = OwnerClass(...)` whose
/// bound local is then called `local(...)`, invoking a changed `__call__` owner
/// indirectly — the tenacity `stop = stop_after_attempt(3); assertTrue(stop(3))`
/// shape that the Tier B judging measured as a false-actionable (#1160). The test
/// genuinely reaches the owner, so the relation is *direct*: surfacing it lets the
/// existing oracle on the bound call (here a broad-boolean smoke `assertTrue`) be
/// reported instead of dropped, correcting the misleading "no direct test exists"
/// diagnosis. This NEVER credits `exposed` — a smoke oracle stays below `Strong`,
/// so the classification remains `weakly_exposed` (matching the sibling
/// `python_broad_boolean_assertion` golden), only the relation/oracle/card change.
///
/// Strictly gated so it never over-links and never collides with `ConstructCall`
/// (the inline `C()(...)` shape, which is checked first):
///   A. the owner is a `__call__` method;
///   B. the test imports the owner's class by name or alias (blocks a same-named
///      class in an unrelated module);
///   C. exactly one real `Class(` construction appears (call boundary, not a
///      comment/string) and it is *not* called inline (inline is `ConstructCall`);
///   D. that construction is a direct assignment `local = Class(` on its line —
///      keyword-arg / wrapper shapes like `Retrying(stop=stop_after_attempt(3))`
///      fail here and stay uncertain;
///   E. the bound `local` is itself called `local(`, and it is assigned exactly
///      once (a reassigned / rebound local is ambiguous and is rejected).
fn local_binding_calls_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    // Guard A: only callable-class `__call__` owners.
    if owner.name != "__call__"
        || !matches!(
            owner.owner_kind,
            Some(OwnerKind::Method | OwnerKind::ClassMethod)
        )
    {
        return false;
    }
    let Some((class_name, _)) = owner.qualified_name.rsplit_once('.') else {
        return false;
    };
    if class_name.is_empty() || !class_name.chars().all(is_python_identifier_char) {
        return false;
    }
    // Guard B: the test must import the owner's class.
    let imports_class = test
        .imports
        .iter()
        .any(|import| import.imported == class_name || import.alias == class_name);
    if !imports_class {
        return false;
    }
    let body = &test.body_text;
    let needle = format!("{class_name}(");
    // Guard C: exactly one real, non-inline `Class(` construction.
    let mut constructions = body.match_indices(&needle).filter(|(idx, _)| {
        has_call_boundary(body, *idx) && !line_prefix_looks_like_comment_or_string(body, *idx)
    });
    let Some((idx, _)) = constructions.next() else {
        return false;
    };
    if constructions.next().is_some() {
        // More than one construction of the class — ambiguous; stay conservative.
        return false;
    }
    // An inline `Class()(...)` is `ConstructCall` territory, not a bound local.
    if construct_result_is_called(body, idx + needle.len() - 1) {
        return false;
    }
    // Guard D: the construction is a direct assignment `local = Class(` on its line.
    let Some(local_var) = binding_target_for_construction(body, idx) else {
        return false;
    };
    // Guard E: the bound local is called, and assigned exactly once.
    contains_call_name(body, &local_var) && assignment_count(body, &local_var) == 1
}

/// Given the byte index of a `Class(` construction, returns the single local
/// variable it is directly assigned to on the same line — `local = Class(` yields
/// `Some("local")`. Returns `None` for keyword-argument (`stop=Class(`), chained
/// (`a = b = Class(`), augmented, attribute-target (`self.x = Class(`), or any
/// non-bare-identifier assignment, so wrapper/dispatch shapes stay uncertain.
fn binding_target_for_construction(body_text: &str, construction_idx: usize) -> Option<String> {
    let line_start = body_text[..construction_idx]
        .rfind('\n')
        .map_or(0, |offset| offset + 1);
    let prefix = body_text[line_start..construction_idx].trim();
    // Require the line to be exactly `<identifier> =` immediately before `Class(`.
    let assign = prefix.strip_suffix('=')?;
    // Reject compound/comparison/augmented operators (`==`, `!=`, `<=`, `+=`, ...).
    if assign.ends_with([
        '=', '!', '<', '>', '+', '-', '*', '/', '%', '&', '|', '^', '~', ':',
    ]) {
        return None;
    }
    let name = assign.trim();
    if name.is_empty()
        || name.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        || !name.chars().all(is_python_identifier_char)
    {
        return None;
    }
    Some(name.to_string())
}

/// Counts direct assignments `name = ...` (whole-token target, not `==`, not an
/// augmented assignment, not a substring of a longer identifier or an attribute
/// like `self.name`) across the test body, so a reassigned binding is rejected.
fn assignment_count(body_text: &str, name: &str) -> usize {
    body_text
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed.strip_prefix(name) else {
                return false;
            };
            // Whole-token boundary: the char after `name` ends the identifier.
            if rest.chars().next().is_some_and(is_python_identifier_char) {
                return false;
            }
            let rest = rest.trim_start();
            rest.starts_with('=') && !rest.starts_with("==")
        })
        .count()
}

fn import_alias_calls_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    // A method/classmethod cannot be imported directly by its bare name, so the
    // `import.imported == owner.name` branch would only ever match a same-named
    // free function in an unrelated module — a false relation that then feeds a
    // false-`exposed`. Restrict that branch to non-method owners.
    let is_method_owner = matches!(
        owner.owner_kind,
        Some(OwnerKind::Method | OwnerKind::ClassMethod)
    );
    test.imports.iter().any(|import| {
        (!is_method_owner
            && import.imported == owner.name
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

fn api_client_route_calls_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    owner
        .route_paths
        .iter()
        .any(|route| body_calls_api_client_route(&test.body_text, route))
}

fn body_calls_api_client_route(body_text: &str, route: &str) -> bool {
    [
        "client.get",
        "client.post",
        "client.put",
        "client.patch",
        "client.delete",
        "client.options",
        "client.head",
    ]
    .into_iter()
    .any(|callee| contains_python_call_with_first_string_argument(body_text, callee, route))
}

fn contains_python_call_with_first_string_argument(
    text: &str,
    callee: &str,
    expected: &str,
) -> bool {
    text.match_indices(callee).any(|(idx, _)| {
        if !python_callee_start_has_boundary(text, idx)
            || python_prefix_hides_code(line_prefix_before(text, idx))
        {
            return false;
        }
        let Some(argument) = first_parenthesized_string_argument(
            text.get(idx + callee.len()..)
                .unwrap_or_default()
                .trim_start(),
        ) else {
            return false;
        };
        argument == expected
    })
}

fn first_parenthesized_string_argument(text: &str) -> Option<String> {
    let body = text.strip_prefix('(')?.trim_start();
    let literal = first_python_string_literal(body)?;
    body.starts_with(&literal)
        .then(|| python_string_literal_value(&literal))
        .flatten()
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

/// Given the byte index of the `(` that opens a `Class(` construction, returns
/// whether its matching `)` is immediately followed (skipping spaces/tabs) by
/// `.method(` — i.e. the constructed instance's method is called inline,
/// `Class(...).method(...)`. Companion to [`construct_result_is_called`] (which
/// detects the `Class()()` callable-instance shape).
fn construct_result_calls_method(text: &str, open_paren_idx: usize, method: &str) -> bool {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut index = open_paren_idx;
    while index < bytes.len() {
        match bytes[index] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    let mut next = index + 1;
                    while matches!(bytes.get(next), Some(b' ' | b'\t')) {
                        next += 1;
                    }
                    return text[next..].starts_with(&format!(".{method}("));
                }
            }
            _ => {}
        }
        index += 1;
    }
    false
}

/// Local names the owner class is known by in `test`: its imported name plus any
/// `as` alias. Empty when the class is not imported — conservative by design, so a
/// class defined elsewhere and never imported cannot lend its identity.
fn owner_class_locals(test: &PythonTest, class: &str) -> Vec<String> {
    let mut locals = Vec::new();
    for import in &test.imports {
        if (import.imported == class || import.alias == class)
            && !import.alias.is_empty()
            && !locals.contains(&import.alias)
        {
            locals.push(import.alias.clone());
        }
    }
    locals
}

/// Whether `body` calls `method` on a receiver statically bound to the owner class
/// (known locally as `local`). Three bound-receiver shapes, all excluding
/// comment/string occurrences:
///   * `Local.method(...)`         — classmethod / direct call on the class;
///   * `Local(...).method(...)`    — inline construct then method call;
///   * `v = Local(...); v.method(...)` — single local binding then method call.
///
/// A bare `.method(` on an unrelated or unresolved receiver is NOT matched: that
/// is the false-`exposed` guard — importing or merely mentioning the owner class
/// is not evidence the asserted method ran on an instance of it.
fn body_calls_method_on_owner_bound_receiver(body: &str, local: &str, method: &str) -> bool {
    // Pattern 1: `Local.method(` — classmethod / direct call on the class itself.
    if contains_attribute_call(body, local, method) {
        return true;
    }
    let construct = format!("{local}(");
    let constructions: Vec<usize> = body
        .match_indices(&construct)
        .filter(|(idx, _)| {
            has_call_boundary(body, *idx) && !line_prefix_looks_like_comment_or_string(body, *idx)
        })
        .map(|(idx, _)| idx)
        .collect();
    // Pattern 2: `Local(...).method(` — inline construct then method call.
    if constructions
        .iter()
        .any(|&idx| construct_result_calls_method(body, idx + construct.len() - 1, method))
    {
        return true;
    }
    // Pattern 3: `v = Local(...); v.method(` — a single unambiguous local binding
    // (reuses the LocalBinding guards: one construction, direct assignment, one
    // assignment of the bound local) then a method call on that local.
    if constructions.len() == 1
        && let Some(var) = binding_target_for_construction(body, constructions[0])
        && assignment_count(body, &var) == 1
        && contains_attribute_call(body, &var, method)
    {
        return true;
    }
    false
}

/// Relation-layer receiver-identity evidence for a method/classmethod owner: a
/// strong observing test calls the owner's method on a receiver statically bound
/// to the owner class (see [`body_calls_method_on_owner_bound_receiver`]). This
/// supersedes the weaker "imports + mentions the owner class" gate, which credited
/// `exposed` whenever the class name merely appeared in the test — even as a dead
/// reference or while the asserted `.method(` ran on an unrelated receiver.
fn strong_test_calls_owner_method_on_bound_receiver(
    owner_class_token: Option<&String>,
    method_name: Option<&String>,
    strong_tests: &[&RelatedTest],
    all_tests: &[PythonTest],
) -> bool {
    let (Some(class), Some(method)) = (owner_class_token, method_name) else {
        return false;
    };
    strong_tests.iter().any(|related_test| {
        all_tests.iter().any(|test| {
            test.name == related_test.name
                && test.file == related_test.file
                && owner_class_locals(test, class).iter().any(|local| {
                    body_calls_method_on_owner_bound_receiver(&test.body_text, local, method)
                })
        })
    })
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
    if contains_dynamic_import(trimmed) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::MissingImportGraph,
            evidence: "static_limit missing_import_graph: changed line uses dynamic import syntax"
                .to_string(),
            missing: "Static limit `missing_import_graph`: the changed line uses dynamic import syntax such as `importlib.import_module(...)` or `__import__(...)`; the Python preview adapter does not build an import graph or resolve imported implementation semantics.".to_string(),
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
    if let Some(decorator) = owner.dynamic_route_decorators.first() {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::DecoratorIndirection,
            evidence: format!(
                "static_limit decorator_indirection: dynamic_route_registration `{decorator}`"
            ),
            missing: format!(
                "Static limit `dynamic_route_registration`: owner `{}` uses dynamic route registration `{decorator}`; syntax-first preview evidence cannot safely match client calls to a concrete route path.",
                owner.qualified_name
            ),
        });
    }
    if let Some(decorator) = owner
        .decorators
        .iter()
        .find(|decorator| !is_transparent_owner_decorator_for_owner(decorator, owner))
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
            evidence: "static_limit mocked_module: related test uses patch/mock/monkeypatch module syntax"
                .to_string(),
            missing: "Static limit `mocked_module`: a related Python test uses patch/mock/monkeypatch module syntax; the preview adapter does not resolve runtime substitution semantics.".to_string(),
        });
    }
    if related_candidates_have_property_based_test_limit(related_candidates) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::PropertyBasedTest,
            evidence: "static_limit property_based_test: related test uses generated inputs"
                .to_string(),
            missing: "Static limit `property_based_test`: a related Python test uses property-based generated inputs such as `@given(...)`; syntax-first preview evidence cannot prove whether the generated cases include the changed discriminator.".to_string(),
        });
    }
    if related_candidates_have_unresolved_pytest_fixture_limit(owner, related_candidates) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::UnresolvedPytestFixture,
            evidence:
                "static_limit unresolved_pytest_fixture: related test uses fixture-sourced values"
                    .to_string(),
            missing: "Static limit `unresolved_pytest_fixture`: a related pytest test depends on fixture-sourced values; syntax-first preview evidence cannot prove whether the fixture supplies the changed discriminator or expected value.".to_string(),
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

fn contains_dynamic_import(text: &str) -> bool {
    contains_python_call_shape(text, "importlib.import_module")
        || contains_python_call_shape(text, "__import__")
}

fn contains_python_call_shape(text: &str, callee: &str) -> bool {
    text.match_indices(callee).any(|(idx, _)| {
        python_callee_start_has_boundary(text, idx)
            && text[idx + callee.len()..].trim_start().starts_with('(')
            && !python_prefix_hides_code(line_prefix_before(text, idx))
    })
}

fn python_callee_start_has_boundary(text: &str, idx: usize) -> bool {
    text[..idx]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_python_identifier_char(ch) && ch != '.')
}

fn line_prefix_before(text: &str, idx: usize) -> &str {
    text[..idx]
        .rsplit_once('\n')
        .map_or(&text[..idx], |(_, line)| line)
}

fn python_prefix_hides_code(prefix: &str) -> bool {
    let mut quote = None;
    let mut escaped = false;
    for ch in prefix.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if ch == '#' {
            return true;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
        }
    }
    quote.is_some()
}

fn contains_metaprogramming(text: &str) -> bool {
    text.contains("__getattr__")
        || text.contains("type(")
        || text.contains("setattr(")
        || contains_metaclass_declaration(text)
}

fn contains_metaclass_declaration(text: &str) -> bool {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("class ") {
        return false;
    }
    contains_python_keyword_assignment_shape(text, "metaclass")
}

fn contains_python_keyword_assignment_shape(text: &str, keyword: &str) -> bool {
    text.match_indices(keyword).any(|(idx, _)| {
        python_callee_start_has_boundary(text, idx)
            && text[idx + keyword.len()..].trim_start().starts_with('=')
            && !python_prefix_hides_code(line_prefix_before(text, idx))
    })
}

fn is_transparent_owner_decorator(decorator: &str) -> bool {
    decorator == "staticmethod"
        || decorator == "classmethod"
        || decorator == "async_def"
        || is_static_route_decorator(decorator)
        || is_static_cli_decorator(decorator)
}

fn is_transparent_owner_decorator_for_owner(decorator: &str, owner: &PythonOwner) -> bool {
    is_transparent_owner_decorator(decorator)
        || is_static_cli_decorator_with_import_context(
            decorator,
            &owner.imports,
            &owner.cli_receiver_names,
        )
}

fn is_static_route_decorator(decorator: &str) -> bool {
    let Some((receiver, method)) = decorator.rsplit_once('.') else {
        return false;
    };
    if !matches!(
        method,
        "get"
            | "post"
            | "put"
            | "patch"
            | "delete"
            | "options"
            | "head"
            | "route"
            | "api_route"
            | "websocket"
    ) {
        return false;
    }

    let Some(receiver_name) = receiver.rsplit('.').next() else {
        return false;
    };
    matches!(
        receiver_name,
        "app" | "api" | "router" | "routes" | "bp" | "blueprint"
    ) || receiver_name.ends_with("_app")
        || receiver_name.ends_with("_api")
        || receiver_name.ends_with("_router")
        || receiver_name.ends_with("_routes")
        || receiver_name.ends_with("_bp")
}

fn is_static_cli_decorator(decorator: &str) -> bool {
    matches!(
        decorator,
        "click.command"
            | "click.group"
            | "click.option"
            | "click.argument"
            | "typer.command"
            | "typer.callback"
    )
}

fn is_static_cli_decorator_with_import_context(
    decorator: &str,
    imports: &[PythonImport],
    cli_receiver_names: &[String],
) -> bool {
    if is_static_cli_decorator(decorator) {
        return true;
    }
    let Some((receiver, method)) = decorator.rsplit_once('.') else {
        return false;
    };
    if !matches!(method, "command" | "callback") || !imports.iter().any(has_typer_import) {
        return false;
    }
    let receiver_name = receiver.rsplit('.').next().unwrap_or(receiver);
    cli_receiver_names
        .iter()
        .any(|candidate| candidate == receiver_name)
}

fn has_typer_import(import: &PythonImport) -> bool {
    import.imported == "typer" && import.alias == "typer"
}

fn collect_static_cli_receiver_names(source: &str, imports: &[PythonImport]) -> Vec<String> {
    if !imports.iter().any(has_typer_import) {
        return Vec::new();
    }
    let mut receivers: Vec<String> = source
        .lines()
        .filter_map(|line| {
            let text = line.trim();
            let (lhs, rhs) = split_python_assignment(text)?;
            if !is_simple_python_identifier(lhs) || !contains_python_call_shape(rhs, "typer.Typer")
            {
                return None;
            }
            Some(lhs.to_string())
        })
        .collect();
    receivers.sort();
    receivers.dedup();
    receivers
}

fn is_simple_python_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
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

fn related_candidates_have_property_based_test_limit(
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> bool {
    related_candidates
        .iter()
        .filter(|candidate| candidate.relation.uses_oracle())
        .any(|candidate| {
            test_uses_property_based_inputs(candidate.test)
                && !candidate.test.assertions.iter().any(|assertion| {
                    assertion.oracle_strength.rank() >= OracleStrength::Strong.rank()
                })
        })
}

fn test_uses_property_based_inputs(test: &PythonTest) -> bool {
    test.decorators.iter().any(|decorator| {
        decorator == "given"
            || decorator.ends_with(".given")
            || decorator == "hypothesis.given"
            || decorator == "example"
            || decorator.ends_with(".example")
    })
}

fn related_candidates_have_unresolved_pytest_fixture_limit(
    owner: &PythonOwner,
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> bool {
    let mut has_unresolved_fixture_relation = false;
    let mut has_concrete_oracle_relation = false;

    for candidate in related_candidates
        .iter()
        .filter(|candidate| candidate.relation.uses_oracle())
    {
        if test_has_unresolved_pytest_fixture_inputs(candidate.test, owner) {
            has_unresolved_fixture_relation = true;
        } else {
            has_concrete_oracle_relation = true;
        }
    }

    has_unresolved_fixture_relation && !has_concrete_oracle_relation
}

fn test_has_unresolved_pytest_fixture_inputs(test: &PythonTest, owner: &PythonOwner) -> bool {
    test.framework == "pytest"
        && !test.parametrized
        && body_calls_owner(&test.body_text, owner)
        && test
            .fixtures
            .iter()
            .filter(|fixture| !is_known_auxiliary_pytest_fixture(fixture))
            .any(|fixture| body_uses_identifier(&test.body_text, fixture))
}

fn is_known_auxiliary_pytest_fixture(fixture: &str) -> bool {
    matches!(
        fixture,
        "capfd"
            | "capfdbinary"
            | "caplog"
            | "capsys"
            | "capsysbinary"
            | "client"
            | "monkeypatch"
            | "mocker"
            | "record_property"
            | "record_testsuite_property"
            | "recwarn"
            | "test_client"
            | "tmp_path"
            | "tmp_path_factory"
            | "tmpdir"
            | "tmpdir_factory"
    )
}

fn body_uses_identifier(body_text: &str, identifier: &str) -> bool {
    let mut line_start = 0;
    for line in body_text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('@')
            && !trimmed.starts_with("def ")
            && !trimmed.starts_with("async def ")
            && line.match_indices(identifier).any(|(idx, _)| {
                let body_idx = line_start + idx;
                has_identifier_boundary(body_text, body_idx, identifier.len())
                    && !line_prefix_looks_like_comment_or_string(body_text, body_idx)
            })
        {
            return true;
        }
        line_start += line.len();
    }
    false
}

fn has_identifier_boundary(body_text: &str, idx: usize, len: usize) -> bool {
    let before = body_text[..idx].chars().next_back();
    let after = body_text[idx + len..].chars().next();
    before.is_none_or(|ch| !is_python_identifier_char(ch))
        && after.is_none_or(|ch| !is_python_identifier_char(ch))
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
            && !line_uses_known_static_cli_symbol(text, import)
            && (text.contains(&format!("{}(", import.alias))
                || text.contains(&format!("{}.", import.alias)))
    })
}

fn is_known_mock_constructor_import(import: &PythonImport) -> bool {
    matches!(import.imported.as_str(), "Mock" | "MagicMock")
        || matches!(import.alias.as_str(), "Mock" | "MagicMock")
}

fn line_uses_known_static_cli_symbol(text: &str, import: &PythonImport) -> bool {
    let alias = import.alias.as_str();
    match import.imported.as_str() {
        "click" if alias == "click" => contains_python_call_shape(text, "click.echo"),
        "typer" if alias == "typer" => contains_python_call_shape(text, "typer.echo"),
        "sys" if alias == "sys" => {
            contains_python_call_shape(text, "sys.exit")
                || contains_python_call_shape(text, "sys.stdout.write")
                || contains_python_call_shape(text, "sys.stderr.write")
        }
        _ => false,
    }
}

fn classify_probe_shape(line_text: &str) -> (ProbeFamily, DeltaKind) {
    let trimmed = line_text.trim_start();
    if is_python_cli_exit_line(trimmed) {
        return (ProbeFamily::SideEffect, DeltaKind::Effect);
    }
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
    if python_return_dict_field_discriminator(trimmed).is_some()
        || python_return_constructor_field_discriminator(trimmed).is_some()
    {
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
        if python_assignment_constructor_field_parts(trimmed).is_some() {
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

fn is_python_cli_exit_line(text: &str) -> bool {
    python_exit_code_discriminator(text).is_some()
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
    owner: &PythonOwner,
    flow_sink: Option<&FlowSinkFact>,
) -> Vec<MissingDiscriminatorFact> {
    let Some(value) = python_missing_discriminator_value(probe_family, line_text, owner) else {
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
    owner: &PythonOwner,
) -> Option<String> {
    match probe_family {
        ProbeFamily::Predicate => python_boundary_discriminator(line_text),
        ProbeFamily::ReturnValue => python_return_value_discriminator(line_text),
        ProbeFamily::ErrorPath => python_exception_discriminator(line_text),
        ProbeFamily::FieldConstruction => python_field_value_discriminator(line_text, owner),
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

fn python_field_value_discriminator(line_text: &str, owner: &PythonOwner) -> Option<String> {
    let text = line_text.trim();
    if let Some((field, value)) = python_return_dict_field_parts(text) {
        if !owner.route_paths.is_empty() {
            return Some(format!("response.json()[\"{field}\"] == {value}"));
        }
        return Some(format!("{field} == {value}"));
    }
    if let Some((_constructor, field, value)) = python_return_constructor_field_parts(text) {
        return Some(format!("result.{field} == {value}"));
    }
    if let Some((target, _constructor, field, value)) =
        python_assignment_constructor_field_parts(text)
    {
        if !owner.route_paths.is_empty() {
            return python_route_response_field_discriminator(&field, &value);
        }
        return Some(format!("{target}.{field} == {value}"));
    }
    let (lhs, rhs) = split_python_assignment(text)?;
    if lhs.is_empty() || rhs.is_empty() {
        return None;
    }
    Some(format!("{lhs} == {rhs}"))
}

fn python_route_response_field_discriminator(field: &str, value: &str) -> Option<String> {
    match field {
        "status" | "status_code" => Some(format!("response.status_code == {value}")),
        "detail" => Some(format!("response.json()[\"detail\"] == {value}")),
        _ => Some(format!("response.{field} == {value}")),
    }
}

fn python_return_dict_field_discriminator(line_text: &str) -> Option<String> {
    let (key, value) = python_return_dict_field_parts(line_text)?;
    Some(format!("{key} == {value}"))
}

fn python_return_constructor_field_discriminator(line_text: &str) -> Option<String> {
    let (_constructor, field, value) = python_return_constructor_field_parts(line_text)?;
    Some(format!("result.{field} == {value}"))
}

fn python_return_dict_field_parts(line_text: &str) -> Option<(String, String)> {
    let expression = line_text.trim().strip_prefix("return ")?.trim();
    let body = expression
        .strip_prefix('{')?
        .trim_start()
        .trim_end_matches('}')
        .trim_end();
    let mut fallback = None;
    for segment in top_level_python_segments(body) {
        let Some((key, value)) = python_dict_field_segment_parts(segment) else {
            continue;
        };
        if fallback.is_none() {
            fallback = Some((key.to_string(), value.to_string()));
        }
        if is_literal_python_model_field_value(value) {
            return Some((key.to_string(), value.to_string()));
        }
    }
    fallback
}

fn top_level_python_segments(text: &str) -> Vec<&str> {
    let mut quote = None;
    let mut escaped = false;
    let mut depth = 0usize;
    let mut segment_start = 0usize;
    let mut segments = Vec::new();
    for (idx, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            continue;
        }
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                segments.push(text[segment_start..idx].trim());
                segment_start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    segments.push(text[segment_start..].trim());
    segments
}

fn python_dict_field_segment_parts(segment: &str) -> Option<(&str, &str)> {
    let colon = top_level_colon(segment)?;
    let key = segment[..colon].trim().trim_matches('"').trim_matches('\'');
    let value = segment[colon + 1..].trim().trim_end_matches('}').trim();
    if key.is_empty() || value.is_empty() {
        None
    } else {
        Some((key, value))
    }
}

fn python_return_constructor_field_parts(line_text: &str) -> Option<(String, String, String)> {
    let expression = line_text.trim().strip_prefix("return ")?.trim();
    let (constructor, args) = split_python_constructor_call(expression)?;
    if !is_python_constructor_callee(constructor) {
        return None;
    }
    let (field, value) = first_python_keyword_argument(args)?;
    if !is_simple_python_model_field_value(value) {
        return None;
    }
    Some((
        constructor.to_string(),
        field.to_string(),
        value.to_string(),
    ))
}

fn python_assignment_constructor_field_parts(
    line_text: &str,
) -> Option<(String, String, String, String)> {
    let (target, expression) = split_python_assignment(line_text.trim())?;
    if !is_simple_python_identifier(target) {
        return None;
    }
    let (constructor, args) = split_python_constructor_call(expression)?;
    if !is_python_constructor_callee(constructor) {
        return None;
    }
    let (field, value) = first_python_keyword_argument(args)?;
    if !is_simple_python_model_field_value(value) {
        return None;
    }
    Some((
        target.to_string(),
        constructor.to_string(),
        field.to_string(),
        value.to_string(),
    ))
}

fn split_python_constructor_call(expression: &str) -> Option<(&str, &str)> {
    let expression = expression.trim();
    if !looks_like_call_expression(expression) {
        return None;
    }
    let open = expression.find('(')?;
    let close = expression.rfind(')')?;
    if close <= open {
        return None;
    }
    let callee = expression[..open].trim();
    let args = expression[open + 1..close].trim();
    (!callee.is_empty() && !args.is_empty()).then_some((callee, args))
}

fn is_python_constructor_callee(callee: &str) -> bool {
    let last_segment = callee.rsplit('.').next().unwrap_or(callee).trim();
    let mut chars = last_segment.chars();
    chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_uppercase())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn first_python_keyword_argument(args: &str) -> Option<(&str, &str)> {
    let mut quote = None;
    let mut escaped = false;
    let mut depth = 0usize;
    let mut segment_start = 0usize;
    for (idx, ch) in args.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            continue;
        }
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                if let Some(parts) = python_keyword_argument_parts(&args[segment_start..idx]) {
                    return Some(parts);
                }
                segment_start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }
    python_keyword_argument_parts(&args[segment_start..])
}

fn python_keyword_argument_parts(segment: &str) -> Option<(&str, &str)> {
    let segment = segment.trim();
    let equals = top_level_equals(segment)?;
    let field = segment[..equals].trim();
    let value = segment[equals + 1..].trim();
    (is_simple_python_identifier(field) && !value.is_empty()).then_some((field, value))
}

fn top_level_equals(text: &str) -> Option<usize> {
    top_level_delimiter(text, '=')
}

fn top_level_colon(text: &str) -> Option<usize> {
    top_level_delimiter(text, ':')
}

fn top_level_delimiter(text: &str, delimiter: char) -> Option<usize> {
    let mut quote = None;
    let mut escaped = false;
    let mut depth = 0usize;
    for (idx, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            continue;
        }
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if ch == delimiter && depth == 0 => return Some(idx),
            _ => {}
        }
    }
    None
}

fn is_simple_python_model_field_value(value: &str) -> bool {
    let value = value.trim();
    is_literal_python_model_field_value(value) || is_simple_python_identifier(value)
}

fn is_literal_python_model_field_value(value: &str) -> bool {
    let value = value.trim();
    python_string_literal_value(value).is_some()
        || matches!(value, "True" | "False" | "None")
        || is_simple_python_numeric_literal(value)
}

fn is_simple_python_numeric_literal(value: &str) -> bool {
    let value = value.trim().strip_prefix('-').unwrap_or(value.trim());
    if value.is_empty() {
        return false;
    }
    let mut digits = 0usize;
    let mut dots = 0usize;
    for ch in value.chars() {
        if ch.is_ascii_digit() {
            digits += 1;
        } else if ch == '.' {
            dots += 1;
            if dots > 1 {
                return false;
            }
        } else {
            return false;
        }
    }
    digits > 0
}

fn python_output_or_call_discriminator(line_text: &str) -> Option<String> {
    let text = line_text.trim();
    if let Some(exit_code) = python_exit_code_discriminator(text) {
        return Some(format!("exit_code == {exit_code}"));
    }
    let literal = first_python_string_literal(text)?;
    if python_stdout_output_call(text) {
        Some(format!("stdout contains {literal}"))
    } else if python_stderr_output_call(text) {
        Some(format!("stderr contains {literal}"))
    } else if python_cli_output_call(text) || text.starts_with("print(") {
        Some(format!("output contains {literal}"))
    } else if text.contains("logger.") || text.contains("logging.") {
        Some(format!("log contains {literal}"))
    } else {
        Some(format!("call includes {literal}"))
    }
}

fn python_cli_output_call(text: &str) -> bool {
    contains_python_call_shape(text, "click.echo") || contains_python_call_shape(text, "typer.echo")
}

fn python_stdout_output_call(text: &str) -> bool {
    contains_python_call_shape(text, "sys.stdout.write")
}

fn python_stderr_output_call(text: &str) -> bool {
    contains_python_call_shape(text, "sys.stderr.write")
}

fn python_exit_code_discriminator(text: &str) -> Option<String> {
    let text = text.trim();
    if let Some(argument) = first_python_call_argument(text, "sys.exit") {
        return normalize_python_exit_code(argument);
    }
    if let Some(rest) = text.strip_prefix("raise SystemExit") {
        let rest = rest.trim_start();
        if let Some(argument) = first_parenthesized_argument(rest) {
            return normalize_python_exit_code(argument);
        }
    }
    None
}

fn first_python_call_argument<'a>(text: &'a str, callee: &str) -> Option<&'a str> {
    let idx = text.find(callee)?;
    if !python_callee_start_has_boundary(text, idx)
        || python_prefix_hides_code(line_prefix_before(text, idx))
    {
        return None;
    }
    first_parenthesized_argument(text.get(idx + callee.len()..)?.trim_start())
}

fn first_parenthesized_argument(text: &str) -> Option<&str> {
    let body = text.strip_prefix('(')?;
    let mut depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    for (idx, ch) in body.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            continue;
        }
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' if depth == 0 => {
                let argument = body[..idx].split(',').next()?.trim();
                return (!argument.is_empty()).then_some(argument);
            }
            ')' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

fn normalize_python_exit_code(argument: &str) -> Option<String> {
    let value = argument.trim();
    if value == "None" {
        return Some("0".to_string());
    }
    if value.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(value.to_string());
    }
    None
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

fn python_string_literal_value(text: &str) -> Option<String> {
    let trimmed = text.trim();
    let mut chars = trimmed.chars();
    let quote = chars.next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    if !trimmed.ends_with(quote) || trimmed.len() < quote.len_utf8() * 2 {
        return None;
    }
    trimmed
        .get(quote.len_utf8()..trimmed.len() - quote.len_utf8())
        .map(str::to_string)
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

/// Significant identifier and string-literal tokens from a changed source line.
/// These approximate the *changed sink* — the attribute/field/value the change
/// touches — so an oracle that asserts on them is observing the change.
fn significant_change_tokens(line_text: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "self", "cls", "return", "if", "elif", "else", "while", "for", "def", "none", "true",
        "false", "and", "or", "not", "in", "is", "raise", "assert", "yield", "await", "async",
        "class", "import", "from", "as", "with", "try", "except", "lambda",
    ];
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in line_text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            current.push(ch);
        } else {
            push_identifier(&mut out, &mut current, STOP);
        }
    }
    push_identifier(&mut out, &mut current, STOP);
    for quote in ['"', '\''] {
        for (index, part) in line_text.split(quote).enumerate() {
            if index % 2 == 1 && part.len() >= 2 {
                out.push(part.to_string());
            }
        }
    }
    out
}

fn push_identifier(out: &mut Vec<String>, current: &mut String, stop: &[&str]) {
    if current.len() >= 3
        && !stop.contains(&current.to_ascii_lowercase().as_str())
        && !current.chars().all(|c| c.is_ascii_digit())
    {
        out.push(current.clone());
    }
    current.clear();
}

/// The visible read-out of the sink-alignment decision. `ripr`'s value over
/// coverage is that a strong oracle credits `exposed` only when it *observes the
/// changed sink*, not merely reaches the owner. This carries which token
/// category the strongest oracle matched so a consumer can see *why* a strong
/// oracle did or did not credit `exposed`. It is a pure read-out: the boolean
/// the classifier uses is derived from `oracle_alignment` (see
/// [`SinkAlignment::observes`]), so the surfaced value can never disagree with
/// the decision.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SinkAlignment {
    changed_sink: Option<String>,
    observed_sink: Option<String>,
    /// One of `direct | alias | changed_sink_token | orthogonal | unknown`.
    oracle_alignment: String,
    alignment_reason: String,
}

impl SinkAlignment {
    /// The decision the classifier uses, derived from the alignment so the two
    /// can never drift. A strong oracle observes the owner when the alignment is
    /// `direct`/`alias`/`changed_sink_token`, OR in the legacy module-owner /
    /// empty-token case (alignment `unknown`, reason `module_owner_no_sink_token`)
    /// which historically returned `true` — the one place where `unknown` does
    /// not imply not-observed, preserved to keep the prior decision unchanged.
    fn observes(&self) -> bool {
        matches!(
            self.oracle_alignment.as_str(),
            "direct" | "alias" | "changed_sink_token"
        ) || self.alignment_reason == "module_owner_no_sink_token"
    }

    /// The alignment surfaced when the classifier did not reach a strong-oracle
    /// branch (no-static-path, static-limit, heuristic-only, or weak-oracle
    /// findings never compute owner alignment). `changed_sink` is retained
    /// because it describes the changed line regardless of the test side.
    fn unknown(changed_sink: Option<String>) -> Self {
        SinkAlignment {
            changed_sink,
            observed_sink: None,
            oracle_alignment: "unknown".to_string(),
            alignment_reason: "no_strong_oracle".to_string(),
        }
    }
}

/// Whether `token` appears in `text` as a whole Python identifier rather than a
/// substring of a larger one. Without this, a common changed-sink token like
/// `buffer` would spuriously "observe" an unrelated oracle that merely contains
/// it (e.g. `buffered_stream` from a different class), over-crediting `exposed` —
/// a confirmed false-exposed vector. Mirrors the identifier-boundary rule of
/// [`has_identifier_boundary`]; whole words such as `key` in `Invalid key` still
/// match, so genuine sink observation is preserved.
fn oracle_text_observes_token(text: &str, token: &str) -> bool {
    if token.is_empty() {
        return false;
    }
    text.match_indices(token)
        .any(|(idx, _)| has_identifier_boundary(text, idx, token.len()))
}

/// Classify whether a strong related-test oracle actually observes the changed
/// behavior's sink — and *which* token category matched. Reach-plus-strong-oracle
/// is not enough: a strong oracle that asserts a *different* value (e.g. a
/// wrapper's return) does not discriminate the change. The three token groups
/// (owner name, import alias, changed-sink tokens) are probed in order against
/// the strongest related oracles, mirroring the prior boolean exactly so the
/// derived decision is unchanged.
fn classify_sink_alignment(
    owner: &PythonOwner,
    line_text: &str,
    related: &[RelatedTest],
    all_tests: &[PythonTest],
) -> SinkAlignment {
    // `changed_sink` describes the changed line; deduped, joined for display.
    let change_tokens = significant_change_tokens(line_text);
    let mut change_display: Vec<String> = Vec::new();
    for token in &change_tokens {
        if !change_display.contains(token) {
            change_display.push(token.clone());
        }
    }
    let changed_sink = if change_display.is_empty() {
        None
    } else {
        Some(change_display.join(", "))
    };

    // The strongest strong-rank related oracle is the one the decision inspects.
    let strong_tests: Vec<&RelatedTest> = related
        .iter()
        .filter(|test| test.oracle_strength.rank() >= OracleStrength::Strong.rank())
        .collect();
    if strong_tests.is_empty() {
        return SinkAlignment::unknown(changed_sink);
    }
    let observed_sink = strong_tests
        .iter()
        .max_by_key(|test| test.oracle_strength.rank())
        .and_then(|test| test.oracle.clone());

    // Owner-name tokens, split for identity-aware crediting. For a method /
    // classmethod owner the bare method name is collision-prone: a same-named
    // method on an unrelated class (`PaymentProcessor.validate` vs owner
    // `TokenValidator.validate`) shares the token `validate` yet is a different
    // entity. Crediting `direct` from the bare method name alone is a silent
    // false-`exposed`, so it requires owner-class identity (the class token is
    // observed, or a strong observing test imports the owner's class). Class
    // tokens and free-function names are unambiguous and credit directly.
    let is_method_owner = matches!(
        owner.owner_kind,
        Some(OwnerKind::Method | OwnerKind::ClassMethod)
    );
    let owner_class_token: Option<String> = if is_method_owner {
        owner
            .qualified_name
            .rsplit_once('.')
            .map(|(class, _)| class)
            .filter(|class| !class.is_empty() && *class != "<module>")
            .map(str::to_string)
    } else {
        None
    };
    // Unambiguous identity tokens: every qualified-name segment except the bare
    // method name of a method owner. For non-method owners this is the full set
    // (a free-function name is its own identity).
    let mut identity_tokens: Vec<String> = owner
        .qualified_name
        .split('.')
        .filter(|token| !token.is_empty() && *token != "<module>")
        .filter(|token| !(is_method_owner && *token == owner.name))
        .map(str::to_string)
        .collect();
    if !is_method_owner && !owner.name.is_empty() && owner.name != "<module>" {
        identity_tokens.push(owner.name.clone());
    }
    // The collision-prone bare method name of a method owner, gated on identity.
    let method_name_token: Option<String> =
        if is_method_owner && !owner.name.is_empty() && owner.name != "<module>" {
            Some(owner.name.clone())
        } else {
            None
        };
    // Import aliases of the owner: `from m import owner as alias` makes the test
    // assert on `alias(...)`, which still observes the owner's output.
    let owner_simple = owner
        .qualified_name
        .rsplit('.')
        .next()
        .unwrap_or(owner.name.as_str());
    let mut alias_tokens: Vec<String> = Vec::new();
    for test in all_tests {
        for import in &test.imports {
            // For a method/classmethod owner the bare method name is not directly
            // importable, so only the owner's CLASS alias is identity-bearing.
            // Matching the bare method name here would credit any same-named free
            // function aliased in an unrelated module (a false-`exposed`).
            let imported_matches = if is_method_owner {
                owner_class_token.as_deref() == Some(import.imported.as_str())
            } else {
                import.imported == owner_simple || import.imported == owner.name
            };
            if imported_matches && !import.alias.is_empty() {
                alias_tokens.push(import.alias.clone());
            }
        }
    }
    let mut change_only = change_tokens.clone();
    identity_tokens.retain(|token| token.len() >= 2);
    let method_name_token = method_name_token.filter(|token| token.len() >= 2);
    alias_tokens.retain(|token| token.len() >= 2);
    change_only.retain(|token| token.len() >= 2);

    // Module owner / no usable token: the prior boolean returned `true` here, so
    // the decision must stay `observes`. Map to `unknown` with the reason that
    // `observes()` special-cases back to true.
    if identity_tokens.is_empty()
        && method_name_token.is_none()
        && alias_tokens.is_empty()
        && change_only.is_empty()
    {
        return SinkAlignment {
            changed_sink,
            observed_sink,
            oracle_alignment: "unknown".to_string(),
            alignment_reason: "module_owner_no_sink_token".to_string(),
        };
    }

    let any_strong_observes = |group: &[String]| -> bool {
        strong_tests.iter().any(|test| {
            test.oracle.as_deref().is_some_and(|text| {
                group
                    .iter()
                    .any(|token| oracle_text_observes_token(text, token))
            })
        })
    };
    // Receiver identity for a method owner: a strong observing test must call the
    // owner's method on a receiver statically bound to the owner class (inline
    // construct, local binding, or a classmethod/direct call on the class). A bare
    // method-name match — even with the owner class imported and mentioned — is not
    // identity-bearing, because the asserted `.method(` may run on an unrelated
    // receiver while the class is referenced (or merely named) elsewhere. This is
    // the false-`exposed` guard at the relation layer.
    let strong_test_binds_method_receiver = strong_test_calls_owner_method_on_bound_receiver(
        owner_class_token.as_ref(),
        method_name_token.as_ref(),
        &strong_tests,
        all_tests,
    );
    let method_name_observed = method_name_token
        .as_ref()
        .is_some_and(|token| any_strong_observes(std::slice::from_ref(token)));
    let (oracle_alignment, alignment_reason) = if any_strong_observes(&identity_tokens) {
        ("direct", "strong_oracle_observes_owner_name")
    } else if method_name_observed && strong_test_binds_method_receiver {
        (
            "direct",
            "strong_oracle_observes_owner_method_on_bound_receiver",
        )
    } else if any_strong_observes(&alias_tokens) {
        ("alias", "strong_oracle_observes_import_alias")
    } else if any_strong_observes(&change_only) {
        (
            "changed_sink_token",
            "strong_oracle_observes_changed_sink_token",
        )
    } else {
        ("orthogonal", "strong_oracle_observes_different_sink")
    };
    SinkAlignment {
        changed_sink,
        observed_sink,
        oracle_alignment: oracle_alignment.to_string(),
        alignment_reason: alignment_reason.to_string(),
    }
}

/// Whether a strong related-test oracle actually observes the changed behavior's
/// sink — not merely reaches the owner. Derived from [`classify_sink_alignment`]
/// so the boolean and the surfaced `oracle_alignment` can never disagree. Now a
/// `#[cfg(test)]` regression helper: production code reads the alignment directly
/// via [`SinkAlignment::observes`], and the existing boolean tests pin that the
/// derivation stays equivalent to the prior decision.
#[cfg(test)]
fn strong_oracle_observes_owner(
    owner: &PythonOwner,
    line_text: &str,
    related: &[RelatedTest],
    all_tests: &[PythonTest],
) -> bool {
    classify_sink_alignment(owner, line_text, related, all_tests).observes()
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
    let alignment = classify_sink_alignment(owner, line_text, &related, all_tests);
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
            } else {
                StageState::Unknown
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
    } else if strongest_strength >= OracleStrength::Strong.rank() && alignment.observes() {
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
    } else if strongest_strength >= OracleStrength::Strong.rank() {
        // A strong oracle reaches the owner, but its assertion observes a value
        // other than the changed owner's output, so static evidence cannot
        // confirm the changed behavior is discriminated. Fail closed to
        // weakly_exposed rather than crediting reach-plus-strong-oracle as
        // discrimination (which would degrade `exposed` back into coverage).
        (
            ExposureClass::WeaklyExposed,
            StageState::Yes,
            StageState::Weak,
            StageState::Weak,
            vec![format!(
                "A strong Python oracle reaches `{}`, but its assertion does not observe the changed owner's output; static evidence cannot confirm the changed behavior is discriminated. Add an assertion on the changed owner's output.",
                owner.name
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

    // Surface the sink alignment only where the classifier actually consulted a
    // strong oracle: the `exposed` branch and the strong-but-orthogonal
    // `weakly_exposed` branch. No-static-path, static-limit, heuristic-only, and
    // weak-oracle findings never computed owner alignment, so they read
    // `unknown` (the `changed_sink` of the changed line is still retained).
    let surfaced_alignment = if matches!(class, ExposureClass::Exposed)
        || (matches!(class, ExposureClass::WeaklyExposed)
            && static_limit.is_none()
            && has_oracle_eligible_relation
            && strongest_strength >= OracleStrength::Strong.rank())
    {
        alignment
    } else {
        SinkAlignment::unknown(alignment.changed_sink.clone())
    };

    let id_path: String = file
        .display()
        .to_string()
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let canonical_gap = static_limit
        .is_none()
        .then(|| canonical_python_gap_for(file, owner, &family, line_text));
    let owner_id = owner.symbol_id();
    let probe_id = fingerprint_probe_id(
        "probe",
        &id_path,
        "python_preview",
        owner_id.0.as_str(),
        &normalize_expression(line_text),
        1,
    );
    let probe = Probe {
        id: probe_id,
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
        python_missing_discriminators(&family, line, line_text, owner, flow_sink.as_ref())
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
    let discriminate_summary = if let Some(limit) = &static_limit {
        format!(
            "Static limit `{}` prevents a safe Python discriminator claim.",
            limit.kind.as_str()
        )
    } else if strongest_strength >= OracleStrength::Strong.rank() {
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
        changed_sink: surfaced_alignment.changed_sink,
        observed_sink: surfaced_alignment.observed_sink,
        oracle_alignment: Some(surfaced_alignment.oracle_alignment),
        alignment_reason: Some(surfaced_alignment.alignment_reason),
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
            if !self.accepts_path(&changed.path)
                || is_detectable_generated_python_file(&changed.path)
            {
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

        let (family, delta) = classify_probe_shape("    return User(active=True)");
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
    fn return_dict_field_parts_prefer_literal_changed_value_candidates() {
        assert_eq!(
            python_return_dict_field_parts("return {\"name\": name, \"status\": \"active\"}"),
            Some(("status".to_string(), "\"active\"".to_string()))
        );
        assert_eq!(
            python_return_dict_field_discriminator(
                "return {\"name\": name, \"status\": \"active\"}"
            )
            .as_deref(),
            Some("status == \"active\"")
        );
        assert_eq!(
            python_return_dict_field_parts(
                "return {\"label\": \"ready, set\", \"status\": status}"
            ),
            Some(("label".to_string(), "\"ready, set\"".to_string()))
        );
        assert_eq!(
            python_return_dict_field_parts("return {\"status\": status}"),
            Some(("status".to_string(), "status".to_string()))
        );
    }

    #[test]
    fn return_dict_field_parts_handle_nested_segments_and_literal_kinds() {
        assert_eq!(
            top_level_python_segments(
                "\"payload\": {\"status\": \"active, pending\"}, \"note\": \"a,b\""
            ),
            vec![
                "\"payload\": {\"status\": \"active, pending\"}",
                "\"note\": \"a,b\""
            ]
        );
        assert_eq!(
            top_level_python_segments("\"label\": \"ready\\\"set\", \"status\": status"),
            vec!["\"label\": \"ready\\\"set\"", "\"status\": status"]
        );
        assert_eq!(
            python_dict_field_segment_parts("\"url\": \"https://example.test/a:b\""),
            Some(("url", "\"https://example.test/a:b\""))
        );
        assert_eq!(python_dict_field_segment_parts("\"status\""), None);
        assert!(is_literal_python_model_field_value("True"));
        assert!(is_literal_python_model_field_value("-1.5"));
        assert!(!is_literal_python_model_field_value("status"));
        assert_eq!(
            python_return_dict_field_parts(
                "return {\"status\": status, invalid_segment, \"count\": total}"
            ),
            Some(("status".to_string(), "status".to_string()))
        );
        assert_eq!(
            python_return_dict_field_parts("return {\"payload\": make_payload(a, b)}"),
            Some(("payload".to_string(), "make_payload(a, b)".to_string()))
        );
        assert_eq!(python_return_dict_field_parts("return {}"), None);
    }

    #[test]
    fn constructor_keyword_field_parts_accept_simple_model_field_values() {
        assert_eq!(
            python_return_constructor_field_parts("return User(active=True)"),
            Some(("User".to_string(), "active".to_string(), "True".to_string()))
        );
        assert_eq!(
            python_return_constructor_field_parts("return models.User(name=\"Ada\")"),
            Some((
                "models.User".to_string(),
                "name".to_string(),
                "\"Ada\"".to_string()
            ))
        );
        assert_eq!(
            python_return_constructor_field_parts("return _User(score=-1.5)"),
            Some(("_User".to_string(), "score".to_string(), "-1.5".to_string()))
        );
        assert_eq!(
            python_return_constructor_field_parts("return User(plan=default_plan)"),
            Some((
                "User".to_string(),
                "plan".to_string(),
                "default_plan".to_string()
            ))
        );
        assert_eq!(
            python_return_constructor_field_parts("return User(label=\"a=b\")"),
            Some((
                "User".to_string(),
                "label".to_string(),
                "\"a=b\"".to_string()
            ))
        );
    }

    #[test]
    fn constructor_keyword_field_parts_fail_closed_for_ambiguous_shapes() {
        assert_eq!(
            python_return_constructor_field_parts("return build_user(active=True)"),
            None
        );
        assert_eq!(
            python_return_constructor_field_parts("return User(\"Ada\")"),
            None
        );
        assert_eq!(
            python_return_constructor_field_parts("return User(profile.active=True)"),
            None
        );
        assert_eq!(
            python_return_constructor_field_parts("return User(active=build_active())"),
            None
        );
        assert_eq!(
            python_return_constructor_field_parts(
                "return User(config={\"active\": True}, active=True)"
            ),
            None
        );
        assert_eq!(
            python_return_constructor_field_parts("value = User(active=True)"),
            None
        );
    }

    #[test]
    fn first_python_keyword_argument_skips_positional_and_nested_arguments() {
        assert_eq!(
            first_python_keyword_argument("factory(a=b), active=True"),
            Some(("active", "True"))
        );
        assert_eq!(
            first_python_keyword_argument("name=\"Ada, Lovelace\", active=True"),
            Some(("name", "\"Ada, Lovelace\""))
        );
        assert_eq!(
            first_python_keyword_argument("metadata={\"a\": \"b,c\"}, active=True"),
            Some(("metadata", "{\"a\": \"b,c\"}"))
        );
        assert_eq!(first_python_keyword_argument("factory(a=b), user"), None);
    }

    #[test]
    fn classify_change_uses_constructor_keyword_field_discriminator() -> Result<(), String> {
        let source = r#"
from dataclasses import dataclass

@dataclass
class User:
    active: bool

def build_user():
    return User(active=True)
"#;
        let owners = extract_owners(Path::new("src/users.py"), source);
        let tests = extract_tests(
            Path::new("tests/test_users.py"),
            r#"
from src.users import build_user

def test_build_user_smoke():
    user = build_user()
    assert user
"#,
        );

        let Some(finding) = classify_change(
            Path::new("src/users.py"),
            9,
            "    return User(active=True)",
            &owners,
            &tests,
        ) else {
            return Err("changed constructor return inside owner should classify".to_string());
        };

        assert_eq!(finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(finding.probe.family, ProbeFamily::FieldConstruction);
        assert_eq!(
            finding
                .activation
                .missing_discriminators
                .first()
                .map(|missing| missing.value.as_str()),
            Some("result.active == True")
        );
        assert!(
            finding
                .evidence
                .iter()
                .any(|entry| entry == "missing_discriminator: result.active == True")
        );
        Ok(())
    }

    #[test]
    fn constructor_keyword_field_helpers_stay_bounded_and_fail_closed() {
        assert_eq!(
            python_return_constructor_field_discriminator("return User(active=True)").as_deref(),
            Some("result.active == True")
        );
        assert_eq!(
            python_return_constructor_field_discriminator("return models.User(score=-1.5)")
                .as_deref(),
            Some("result.score == -1.5")
        );
        assert_eq!(
            split_python_constructor_call("User(active=True)"),
            Some(("User", "active=True"))
        );
        assert_eq!(split_python_constructor_call("User()"), None);
        assert_eq!(split_python_constructor_call("(User(active=True))"), None);
        assert!(is_python_constructor_callee("models.User"));
        assert!(is_python_constructor_callee("_PrivateUser"));
        assert!(!is_python_constructor_callee("make_user"));
        assert_eq!(
            first_python_keyword_argument("ignored, active=True"),
            Some(("active", "True"))
        );
        assert_eq!(
            first_python_keyword_argument("label=\"a,b=c\", active=False"),
            Some(("label", "\"a,b=c\""))
        );
        assert_eq!(
            first_python_keyword_argument("meta={\"threshold\": \"a=b,c\"}"),
            Some(("meta", "{\"threshold\": \"a=b,c\"}"))
        );
        assert_eq!(python_keyword_argument_parts("not keyword"), None);
        assert_eq!(top_level_equals("metadata={\"a\": \"b=c\"}"), Some(8));
        assert_eq!(top_level_equals("metadata"), None);
        assert!(is_simple_python_model_field_value("\"active\""));
        assert!(is_simple_python_model_field_value("True"));
        assert!(is_simple_python_model_field_value("None"));
        assert!(is_simple_python_model_field_value("-1.25"));
        assert!(is_simple_python_model_field_value(".5"));
        assert!(!is_simple_python_model_field_value("."));
        assert!(!is_simple_python_model_field_value("-"));
        assert!(!is_simple_python_model_field_value("1.2.3"));
        assert!(!is_simple_python_model_field_value("make_value()"));
        assert_eq!(
            python_return_constructor_field_discriminator("return make_user(active=True)"),
            None
        );
        assert_eq!(
            python_return_constructor_field_discriminator("return User(active=make_value())"),
            None
        );
        assert_eq!(
            python_assignment_constructor_field_parts(
                "response = Response(status_code=422, detail=\"coupon expired\")"
            ),
            Some((
                "response".to_string(),
                "Response".to_string(),
                "status_code".to_string(),
                "422".to_string()
            ))
        );
        assert_eq!(
            python_assignment_constructor_field_parts("response.body = Response(status_code=422)"),
            None
        );
        assert_eq!(
            python_assignment_constructor_field_parts("response = make_response(status_code=422)"),
            None
        );
        assert_eq!(
            python_assignment_constructor_field_parts("response = Response(detail=message())"),
            None
        );
        assert_eq!(
            python_route_response_field_discriminator("status_code", "422").as_deref(),
            Some("response.status_code == 422")
        );
        assert_eq!(
            python_route_response_field_discriminator("detail", "\"coupon expired\"").as_deref(),
            Some("response.json()[\"detail\"] == \"coupon expired\"")
        );
        assert_eq!(
            python_route_response_field_discriminator("headers", "expected_headers").as_deref(),
            Some("response.headers == expected_headers")
        );
    }

    #[test]
    fn construct_result_is_called_distinguishes_inline_from_bound() {
        // open-paren index of the first `(` in each fixture string (always present).
        let at = |s: &str| s.find('(').unwrap_or(0);
        // Inline construct-call `C(...)(...)`: the constructed instance is called.
        let inline = "Renderer()(None, event)";
        assert!(construct_result_is_called(inline, at(inline)));
        let inline_args = "Renderer(sort=True)(event)";
        assert!(construct_result_is_called(inline_args, at(inline_args)));
        // Bound local `x = C(...)` then a separate `x(...)`: NOT an inline call —
        // the constructor's `)` is followed by a newline, not `(`. Keeps the
        // local-callable case uncertain (consistent with #1221).
        let bound = "stop = stop_after_attempt(3)\n    stop(3)";
        assert!(!construct_result_is_called(bound, at(bound)));
        // Plain construction with no following call.
        let plain = "r = Renderer()";
        assert!(!construct_result_is_called(plain, at(plain)));
    }

    fn call_owner(owners: &[PythonOwner]) -> Result<&PythonOwner, String> {
        owners
            .iter()
            .find(|owner| owner.name == "__call__")
            .ok_or_else(|| "fixture defines a __call__ owner".to_string())
    }

    const STOP_SOURCE: &str = "class stop_after_attempt:\n    def __init__(self, max_attempt_number):\n        self.max_attempt_number = max_attempt_number\n\n    def __call__(self, attempt_number):\n        return attempt_number >= self.max_attempt_number\n";

    #[test]
    fn local_binding_relation_links_direct_and_surfaces_smoke_oracle() -> Result<(), String> {
        // The tenacity false-actionable shape: `stop = stop_after_attempt(3)`
        // bound once and called via `stop(3)` under a broad-boolean smoke oracle.
        let owners = extract_owners(Path::new("src/stop.py"), STOP_SOURCE);
        let tests = extract_tests(
            Path::new("tests/test_stop.py"),
            "import unittest\n\nfrom src.stop import stop_after_attempt\n\n\nclass StopTest(unittest.TestCase):\n    def test_stop_after_attempt(self):\n        stop = stop_after_attempt(3)\n        self.assertTrue(stop(3))\n",
        );
        let owner = call_owner(&owners)?;
        assert_eq!(
            related_test_relation(&tests[0], owner),
            Some(PythonRelationKind::LocalBinding),
            "single bound local called as `stop(3)` should link directly via local_binding"
        );

        let Some(finding) = classify_change(
            Path::new("src/stop.py"),
            6,
            "        return attempt_number >= self.max_attempt_number",
            &owners,
            &tests,
        ) else {
            return Err("changed return inside __call__ should classify".to_string());
        };
        // Must STAY weakly_exposed — a smoke oracle never credits `exposed`.
        assert_eq!(finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Smoke,
            "the assertTrue(stop(3)) smoke oracle must be surfaced, not dropped to unknown"
        );
        Ok(())
    }

    #[test]
    fn local_binding_does_not_fire_for_inline_construct_call() -> Result<(), String> {
        // Inline `C()(...)` is ConstructCall territory, not a bound local.
        let owners = extract_owners(Path::new("src/stop.py"), STOP_SOURCE);
        let tests = extract_tests(
            Path::new("tests/test_stop.py"),
            "from src.stop import stop_after_attempt\n\n\ndef test_inline():\n    assert stop_after_attempt(3)(3)\n",
        );
        let owner = call_owner(&owners)?;
        assert_eq!(
            related_test_relation(&tests[0], owner),
            Some(PythonRelationKind::ConstructCall),
            "inline construct-call must stay ConstructCall, not LocalBinding"
        );
        Ok(())
    }

    #[test]
    fn local_binding_does_not_fire_for_reassigned_binding() -> Result<(), String> {
        // A rebound local is ambiguous: which construction is called is unclear.
        let owners = extract_owners(Path::new("src/stop.py"), STOP_SOURCE);
        let tests = extract_tests(
            Path::new("tests/test_stop.py"),
            "from src.stop import stop_after_attempt\n\n\ndef test_reassigned():\n    stop = stop_after_attempt(3)\n    stop = stop_after_attempt(4)\n    assert stop(3)\n",
        );
        let owner = call_owner(&owners)?;
        assert!(
            !local_binding_calls_owner(&tests[0], owner),
            "two constructions / reassigned binding must not link via local_binding"
        );
        Ok(())
    }

    #[test]
    fn local_binding_does_not_fire_for_wrapper_keyword_argument() -> Result<(), String> {
        // `Retrying(stop=stop_after_attempt(3))` binds a wrapper, not the class —
        // the assignment target is `retrying`, not `stop_after_attempt(...)`.
        let owners = extract_owners(Path::new("src/stop.py"), STOP_SOURCE);
        let tests = extract_tests(
            Path::new("tests/test_stop.py"),
            "from src.stop import stop_after_attempt\nimport tenacity\n\n\ndef test_wrapper():\n    retrying = tenacity.Retrying(stop=stop_after_attempt(3))\n    assert retrying(lambda: None)\n",
        );
        let owner = call_owner(&owners)?;
        assert!(
            !local_binding_calls_owner(&tests[0], owner),
            "a keyword-argument construction inside a wrapper must not link via local_binding"
        );
        Ok(())
    }

    #[test]
    fn local_binding_requires_importing_the_owner_class() -> Result<(), String> {
        // Guard B: a same-named local without importing the class must not link.
        let owners = extract_owners(Path::new("src/stop.py"), STOP_SOURCE);
        let tests = extract_tests(
            Path::new("tests/test_stop.py"),
            "def test_no_import():\n    stop = stop_after_attempt(3)\n    assert stop(3)\n",
        );
        let owner = call_owner(&owners)?;
        assert!(
            !local_binding_calls_owner(&tests[0], owner),
            "without importing the owner class, local_binding must not fire (Guard B)"
        );
        Ok(())
    }

    #[test]
    fn binding_target_extracts_only_direct_identifier_assignment() {
        // Direct `local = Class(` extracts the bare identifier.
        let direct = "    stop = stop_after_attempt(3)\n";
        let idx = direct.find("stop_after_attempt(").unwrap_or(0);
        assert_eq!(
            binding_target_for_construction(direct, idx).as_deref(),
            Some("stop")
        );
        // Keyword argument is not an assignment target.
        let kwarg = "    retrying = Retrying(stop=stop_after_attempt(3))\n";
        let kidx = kwarg.find("stop_after_attempt(").unwrap_or(0);
        assert_eq!(binding_target_for_construction(kwarg, kidx), None);
        // Attribute target `self.x = Class(` is not a bare local.
        let attr = "    self.stop = stop_after_attempt(3)\n";
        let aidx = attr.find("stop_after_attempt(").unwrap_or(0);
        assert_eq!(binding_target_for_construction(attr, aidx), None);
    }

    #[test]
    fn oracle_text_observes_token_requires_identifier_boundary() {
        // Whole-word matches still observe (preserves genuine sink alignment).
        assert!(oracle_text_observes_token(
            "assert raises(ValueError, match='Invalid key: x')",
            "key"
        ));
        assert!(oracle_text_observes_token("assert stop(3)", "stop"));
        assert!(oracle_text_observes_token(
            "assert x.max_buffer_size == 2",
            "max_buffer_size"
        ));
        // Substring co-occurrence must NOT observe: the confirmed false-exposed
        // vector — `buffer` (a changed-sink token) inside an unrelated
        // `buffered_stream` oracle from a different class.
        assert!(!oracle_text_observes_token(
            "assert buffered_stream.receive_exactly(10) == b\"x\"",
            "buffer"
        ));
        assert!(!oracle_text_observes_token("assert client.send()", "len"));
        assert!(!oracle_text_observes_token("assert keys() == []", "key"));
        assert!(!oracle_text_observes_token("anything", ""));
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

    const AUTH_SOURCE: &str = "class TokenValidator:\n    def __init__(self, valid):\n        self._valid = valid\n\n    def validate(self, token):\n        return token.strip() in self._valid\n";
    const AUTH_CHANGED_LINE: &str = "        return token.strip() in self._valid";

    #[test]
    fn method_name_without_class_identity_stays_orthogonal() -> Result<(), String> {
        // False-`exposed` guard: the only related test exercises a DIFFERENT
        // class's same-named method (`PaymentProcessor.validate`) and never
        // imports the owner's class. The bare method-name token `validate` must
        // not credit `direct` alignment without owner-class identity.
        let owners = extract_owners(Path::new("src/auth.py"), AUTH_SOURCE);
        let tests = extract_tests(
            Path::new("tests/test_billing.py"),
            "from src.billing import PaymentProcessor\n\n\ndef test_billing_validate():\n    proc = PaymentProcessor()\n    assert proc.validate(\"card1234 \") == True\n",
        );
        let Some(finding) = classify_change(
            Path::new("src/auth.py"),
            6,
            AUTH_CHANGED_LINE,
            &owners,
            &tests,
        ) else {
            return Err("changed return inside validate should classify".to_string());
        };
        assert_eq!(
            finding.class,
            ExposureClass::WeaklyExposed,
            "a same-named method on an unrelated class must not credit exposed"
        );
        assert_eq!(finding.oracle_alignment.as_deref(), Some("orthogonal"));
        Ok(())
    }

    #[test]
    fn method_name_with_class_import_identity_credits_exposed() -> Result<(), String> {
        // Identity preserved: the test imports and constructs the owner's class,
        // then observes its method under an exact-value oracle. The bare
        // method-name match is legitimate here, so `direct`/`exposed` stands.
        let owners = extract_owners(Path::new("src/auth.py"), AUTH_SOURCE);
        let tests = extract_tests(
            Path::new("tests/test_auth.py"),
            "from src.auth import TokenValidator\n\n\ndef test_auth_validate():\n    validator = TokenValidator([\"card1234\"])\n    assert validator.validate(\"card1234\") == True\n",
        );
        let Some(finding) = classify_change(
            Path::new("src/auth.py"),
            6,
            AUTH_CHANGED_LINE,
            &owners,
            &tests,
        ) else {
            return Err("changed return inside validate should classify".to_string());
        };
        assert_eq!(
            finding.class,
            ExposureClass::Exposed,
            "a test that imports and exercises the owner class keeps exposed credit"
        );
        assert_eq!(finding.oracle_alignment.as_deref(), Some("direct"));
        Ok(())
    }

    #[test]
    fn method_name_dead_import_does_not_credit_exposed() -> Result<(), String> {
        // Bypass guard: a DEAD import of the owner class (never used in the test
        // body) is not identity evidence. The test exercises a different class's
        // same-named method, so it must stay conservative.
        let owners = extract_owners(Path::new("src/auth.py"), AUTH_SOURCE);
        let tests = extract_tests(
            Path::new("tests/test_billing.py"),
            "from src.billing import PaymentProcessor\nfrom src.auth import TokenValidator\n\n\ndef test_billing_validate():\n    proc = PaymentProcessor()\n    assert proc.validate(\"card1234 \") == True\n",
        );
        let Some(finding) = classify_change(
            Path::new("src/auth.py"),
            6,
            AUTH_CHANGED_LINE,
            &owners,
            &tests,
        ) else {
            return Err("changed return inside validate should classify".to_string());
        };
        assert_eq!(
            finding.class,
            ExposureClass::WeaklyExposed,
            "a dead import of the owner class must not credit exposed"
        );
        assert_ne!(finding.oracle_alignment.as_deref(), Some("direct"));
        Ok(())
    }

    #[test]
    fn method_owner_free_function_alias_does_not_credit_exposed() -> Result<(), String> {
        // Bypass guard: a same-named FREE function aliased from an unrelated
        // module must not credit `exposed`/`alias` for a method owner. The test
        // never imports or exercises the owner's class.
        let owners = extract_owners(Path::new("src/auth.py"), AUTH_SOURCE);
        let tests = extract_tests(
            Path::new("tests/test_helpers.py"),
            "from src.helpers import validate as run_check\n\n\ndef test_helpers_run_check():\n    assert run_check(\"data\") == True\n",
        );
        if let Some(finding) = classify_change(
            Path::new("src/auth.py"),
            6,
            AUTH_CHANGED_LINE,
            &owners,
            &tests,
        ) {
            assert_ne!(
                finding.class,
                ExposureClass::Exposed,
                "a same-named free-function alias must not credit exposed for a method owner"
            );
            assert_ne!(finding.oracle_alignment.as_deref(), Some("alias"));
        }
        Ok(())
    }

    #[test]
    fn method_name_class_constructed_but_method_on_other_receiver_does_not_credit_exposed()
    -> Result<(), String> {
        // Receiver-identity guard (the residual leak the #1253 class-import gate
        // left open): the owner class is imported AND constructed in real code, but
        // the strong oracle's `.validate(` runs on an UNRELATED receiver. Class
        // identity is present; receiver identity is not, so it must stay
        // conservative.
        let owners = extract_owners(Path::new("src/auth.py"), AUTH_SOURCE);
        let tests = extract_tests(
            Path::new("tests/test_billing.py"),
            "from src.auth import TokenValidator\nfrom src.billing import PaymentProcessor\n\n\ndef test_billing_validate():\n    reference = TokenValidator([\"card1234\"])\n    proc = PaymentProcessor()\n    assert proc.validate(\"card1234 \") == True\n",
        );
        let Some(finding) = classify_change(
            Path::new("src/auth.py"),
            6,
            AUTH_CHANGED_LINE,
            &owners,
            &tests,
        ) else {
            return Err("changed return inside validate should classify".to_string());
        };
        assert_eq!(
            finding.class,
            ExposureClass::WeaklyExposed,
            "constructing the owner class is not enough; the asserted method ran on a different receiver"
        );
        assert_ne!(finding.oracle_alignment.as_deref(), Some("direct"));
        Ok(())
    }

    #[test]
    fn method_name_inline_construct_call_credits_exposed() -> Result<(), String> {
        // Positive control: an inline `OwnerClass(...).method(...)` binds the
        // receiver to the owner class, and a strong exact-value oracle observes it.
        let owners = extract_owners(Path::new("src/auth.py"), AUTH_SOURCE);
        let tests = extract_tests(
            Path::new("tests/test_auth.py"),
            "from src.auth import TokenValidator\n\n\ndef test_auth_inline():\n    assert TokenValidator([\"card1234\"]).validate(\"card1234\") == True\n",
        );
        let Some(finding) = classify_change(
            Path::new("src/auth.py"),
            6,
            AUTH_CHANGED_LINE,
            &owners,
            &tests,
        ) else {
            return Err("changed return inside validate should classify".to_string());
        };
        assert_eq!(
            finding.class,
            ExposureClass::Exposed,
            "inline construct-call binds the receiver to the owner class"
        );
        assert_eq!(finding.oracle_alignment.as_deref(), Some("direct"));
        Ok(())
    }

    #[test]
    fn method_name_classmethod_direct_call_credits_exposed() -> Result<(), String> {
        // Positive control: a classmethod called directly on the owner class
        // (`OwnerClass.method(...)`) is receiver-bound by construction.
        let source = "class TokenRegistry:\n    @classmethod\n    def lookup(cls, token):\n        return token.strip()\n";
        let owners = extract_owners(Path::new("src/registry.py"), source);
        let tests = extract_tests(
            Path::new("tests/test_registry.py"),
            "from src.registry import TokenRegistry\n\n\ndef test_registry_lookup():\n    assert TokenRegistry.lookup(\"abc \") == \"abc\"\n",
        );
        let Some(finding) = classify_change(
            Path::new("src/registry.py"),
            4,
            "        return token.strip()",
            &owners,
            &tests,
        ) else {
            return Err("changed return inside lookup should classify".to_string());
        };
        assert_eq!(
            finding.class,
            ExposureClass::Exposed,
            "a classmethod called on the owner class is receiver-bound"
        );
        assert_eq!(finding.oracle_alignment.as_deref(), Some("direct"));
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
    fn classify_exception_match_assertion_as_exposed() -> Result<(), String> {
        let finding = classify_change(
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
                 def test_rejects_zero_value():\n    with pytest.raises(ValueError, match=\"positive required\"):\n        require_positive(0)\n",
            ),
        )
        .ok_or_else(|| "exception-path change should classify".to_string())?;
        assert_eq!(finding.class, ExposureClass::Exposed);
        assert!(missing_discriminator_values(&finding).is_empty());
        assert_eq!(
            finding.related_tests[0].oracle_kind,
            OracleKind::ExactErrorVariant
        );
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Strong
        );
        Ok(())
    }

    #[test]
    fn classify_field_assertion_as_exposed() -> Result<(), String> {
        let finding = classify_change(
            Path::new("src/invoice.py"),
            2,
            "    return {\"status\": \"paid\", \"id\": invoice_id}",
            &extract_owners(
                Path::new("src/invoice.py"),
                "def invoice_payload(invoice_id):\n    return {\"status\": \"paid\", \"id\": invoice_id}\n",
            ),
            &extract_tests(
                Path::new("tests/test_invoice.py"),
                "from src.invoice import invoice_payload\n\n\
                 def test_invoice_payload_status():\n    payload = invoice_payload(\"inv-123\")\n    assert payload[\"status\"] == \"paid\"\n",
            ),
        )
        .ok_or_else(|| "field-value change should classify".to_string())?;
        assert_eq!(finding.class, ExposureClass::Exposed);
        assert!(missing_discriminator_values(&finding).is_empty());
        assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Strong
        );
        Ok(())
    }

    #[test]
    fn classify_output_assertion_as_exposed() -> Result<(), String> {
        let finding = classify_change(
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
                 def test_warn_coupon_exact_output(caplog):\n    warn_coupon()\n    assert caplog.text == \"coupon expired\"\n",
            ),
        )
        .ok_or_else(|| "output/log change should classify".to_string())?;
        assert_eq!(finding.class, ExposureClass::Exposed);
        assert!(missing_discriminator_values(&finding).is_empty());
        assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Strong
        );
        Ok(())
    }

    #[test]
    fn classify_click_output_change_as_repairable_cli_gap() -> Result<(), String> {
        let finding = classify_change(
            Path::new("src/commands.py"),
            5,
            "    click.echo(\"shipment queued\")",
            &extract_owners(
                Path::new("src/commands.py"),
                "import click\n\n@click.command()\ndef ship():\n    click.echo(\"shipment queued\")\n",
            ),
            &extract_tests(
                Path::new("tests/test_commands.py"),
                "from src.commands import ship\n\n\
                 def test_ship_smoke(capsys):\n    ship()\n    captured = capsys.readouterr()\n    assert captured.out\n",
            ),
        )
        .ok_or_else(|| "click output change should classify".to_string())?;

        assert_eq!(finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(finding.static_limit_kind, None);
        assert_eq!(
            missing_discriminator_values(&finding),
            vec!["output contains \"shipment queued\""]
        );
        assert_eq!(
            evidence_value(&finding, "suggested_verify_command: "),
            Some("pytest tests/test_commands.py::test_ship_smoke")
        );
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
        // With content-addressed ids, moving the owner to a different line
        // (without changing the expression) must NOT change the probe id.
        assert_eq!(
            before_finding.probe.id, after_finding.probe.id,
            "content-addressed id must be stable across line movement"
        );
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
        let property_based_tests = extract_tests(
            Path::new("tests/test_service.py"),
            "from hypothesis import given, strategies as st\nfrom src.service import total\n\n@given(st.integers())\ndef test_total_property_based(value):\n    assert total(value) >= 0\n",
        );
        let property_based_candidates =
            related_test_candidates(&plain_owner, &property_based_tests);
        let property_based_with_exact_tests = {
            let mut tests = property_based_tests.clone();
            tests.extend(extract_tests(
                Path::new("tests/test_service_exact.py"),
                "from src.service import total\n\ndef test_total_exact():\n    assert total(1) == 1\n",
            ));
            tests
        };
        let property_based_with_exact_candidates =
            related_test_candidates(&plain_owner, &property_based_with_exact_tests);
        let unresolved_fixture_tests = extract_tests(
            Path::new("tests/test_service.py"),
            "from src.service import total\n\ndef test_total_fixture_case(case):\n    assert total(case.value) == case.expected\n",
        );
        let unresolved_fixture_candidates =
            related_test_candidates(&plain_owner, &unresolved_fixture_tests);
        let unresolved_fixture_with_exact_tests = {
            let mut tests = unresolved_fixture_tests.clone();
            tests.extend(extract_tests(
                Path::new("tests/test_service_exact.py"),
                "from src.service import total\n\ndef test_total_exact():\n    assert total(1) == 1\n",
            ));
            tests
        };
        let unresolved_fixture_with_exact_candidates =
            related_test_candidates(&plain_owner, &unresolved_fixture_with_exact_tests);
        let parametrized_tests = extract_tests(
            Path::new("tests/test_service.py"),
            "import pytest\nfrom src.service import total\n\n@pytest.mark.parametrize(\"value, expected\", [(1, 1)])\ndef test_total_parametrized(value, expected):\n    assert total(value) == expected\n",
        );
        let parametrized_candidates = related_test_candidates(&plain_owner, &parametrized_tests);
        let property_based_with_same_test_exact_tests = extract_tests(
            Path::new("tests/test_service.py"),
            "from hypothesis import given, strategies as st\nfrom src.service import total\n\n@given(st.integers())\ndef test_total_property_based(value):\n    assert total(1) == 1\n",
        );
        let property_based_with_same_test_exact_candidates =
            related_test_candidates(&plain_owner, &property_based_with_same_test_exact_tests);
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
            static_limit_for_change("    return 1", &plain_owner, &property_based_candidates)
                .map(|limit| limit.kind),
            Some(StaticLimitKind::PropertyBasedTest)
        );
        assert_eq!(
            static_limit_for_change(
                "    return 1",
                &plain_owner,
                &property_based_with_exact_candidates
            )
            .map(|limit| limit.kind),
            Some(StaticLimitKind::PropertyBasedTest)
        );
        assert_eq!(
            static_limit_for_change(
                "    return 1",
                &plain_owner,
                &property_based_with_same_test_exact_candidates
            )
            .map(|limit| limit.kind),
            None
        );
        assert_eq!(
            static_limit_for_change("    return 1", &plain_owner, &unresolved_fixture_candidates)
                .map(|limit| limit.kind),
            Some(StaticLimitKind::UnresolvedPytestFixture)
        );
        assert_eq!(
            static_limit_for_change(
                "    return 1",
                &plain_owner,
                &unresolved_fixture_with_exact_candidates
            )
            .map(|limit| limit.kind),
            None
        );
        assert_eq!(
            static_limit_for_change("    return 1", &plain_owner, &parametrized_candidates)
                .map(|limit| limit.kind),
            None
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
        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Unknown);
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
    fn classify_change_property_based_test_fails_closed() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            "from hypothesis import given, strategies as st\nfrom src.pricing import apply_discount\n\n@given(st.integers(min_value=0), st.integers(min_value=0))\ndef test_apply_discount_property(amount, threshold):\n    assert apply_discount(amount, threshold) <= amount\n",
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
            Some(StaticLimitKind::PropertyBasedTest)
        );
        assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
        assert!(finding.canonical_gap.is_none());
        assert!(finding.recommended_next_step.is_none());
        assert!(finding.activation.missing_discriminators.is_empty());
        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Unknown);
        assert!(
            finding
                .evidence
                .iter()
                .any(|entry| entry.starts_with("static_limit property_based_test:"))
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|entry| entry.contains("Static limit `property_based_test`"))
        );
        Ok(())
    }

    #[test]
    fn classify_change_unresolved_pytest_fixture_fails_closed() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount, threshold):\n    if amount >= threshold:\n        return amount - 10\n    return amount\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            "from src.pricing import apply_discount\n\ndef test_apply_discount_fixture_case(discount_case):\n    assert apply_discount(discount_case.amount, discount_case.threshold) == discount_case.expected\n",
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
            Some(StaticLimitKind::UnresolvedPytestFixture)
        );
        assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
        assert!(finding.canonical_gap.is_none());
        assert!(finding.recommended_next_step.is_none());
        assert!(finding.activation.missing_discriminators.is_empty());
        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Unknown);
        assert!(
            finding
                .evidence
                .iter()
                .any(|entry| entry.starts_with("static_limit unresolved_pytest_fixture:"))
        );
        assert!(
            finding
                .missing
                .iter()
                .any(|entry| entry.contains("Static limit `unresolved_pytest_fixture`"))
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
        assert_eq!(finding.ripr.reveal.discriminate.state, StageState::Unknown);
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
            resolve_tsconfig_paths: false,
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
            resolve_tsconfig_paths: false,
        };
        let policy = OraclePolicy::default();
        let result = adapter.analyze_repo(&options, &policy)?;
        assert!(result.findings.is_empty());
        assert_eq!(result.production_files, 0);
        Ok(())
    }
}
