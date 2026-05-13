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
    Confidence, DeltaKind, ExposureClass, Finding, LanguageId as DomainLanguageId, LanguageStatus,
    OracleKind, OracleStrength, OwnerKind, Probe, ProbeFamily, ProbeId, RelatedTest,
    RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState, StaticLimitKind,
};
use rustpython_parser::{
    Mode,
    ast::{self, Expr, Mod, Stmt},
    parse,
    text_size::TextRange,
};
use std::path::{Path, PathBuf};

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
    owner_kind: OwnerKind,
    decorators: Vec<String>,
    imports: Vec<PythonImport>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PythonTest {
    name: String,
    file: PathBuf,
    line: usize,
    body_text: String,
    imports: Vec<PythonImport>,
    decorators: Vec<String>,
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
}

fn parse_module(path: &Path, source: &str) -> Option<Mod> {
    let source_path = path.to_string_lossy();
    let module = parse(source, Mode::Module, source_path.as_ref()).ok()?;
    match module {
        Mod::Module(_) => Some(module),
        _ => None,
    }
}

/// 1-indexed line for a 0-indexed byte offset.
fn line_for_offset(source: &str, offset: usize) -> usize {
    let mut line: usize = 1;
    for (idx, ch) in source.char_indices() {
        if idx >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
        }
    }
    line
}

fn line_for_range_start(source: &str, range: TextRange) -> usize {
    line_for_offset(source, usize::from(range.start()))
}

fn line_for_range_end(source: &str, range: TextRange) -> usize {
    line_for_offset(source, usize::from(range.end()))
}

fn text_for_range(source: &str, range: TextRange) -> String {
    let start = usize::from(range.start()).min(source.len());
    let end = usize::from(range.end()).min(source.len());
    source.get(start..end).unwrap_or_default().to_string()
}

fn normalized_path(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    normalized
}

fn is_test_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if file_name.starts_with("test_") || file_name.ends_with("_test.py") {
        return true;
    }
    path.components().any(|component| {
        let text = component.as_os_str().to_string_lossy();
        text == "tests" || text == "test"
    })
}

fn extract_owners(file: &Path, source: &str) -> Vec<PythonOwner> {
    let Some(Mod::Module(module)) = parse_module(file, source) else {
        return Vec::new();
    };
    let mut owners = Vec::new();
    let imports = collect_imports_from_statements(&module.body);
    collect_owners_from_statements(file, source, &module.body, None, &imports, &mut owners);
    owners
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
        owner_kind,
        decorators,
        imports: context.imports.to_vec(),
    }
}

fn extract_tests(file: &Path, source: &str) -> Vec<PythonTest> {
    let Some(Mod::Module(module)) = parse_module(file, source) else {
        return Vec::new();
    };
    let mut tests = Vec::new();
    let imports = collect_imports_from_statements(&module.body);
    collect_tests_from_statements(file, source, &module.body, false, &imports, &mut tests);
    tests
}

fn collect_tests_from_statements(
    file: &Path,
    source: &str,
    statements: &[Stmt],
    in_unittest_class: bool,
    imports: &[PythonImport],
    out: &mut Vec<PythonTest>,
) {
    for stmt in statements {
        match stmt {
            Stmt::FunctionDef(function) if function.name.as_str().starts_with("test_") => {
                out.push(PythonTest {
                    name: function.name.to_string(),
                    file: file.to_path_buf(),
                    line: line_for_range_start(source, function.range),
                    body_text: text_for_range(source, function.range),
                    imports: imports.to_vec(),
                    decorators: decorator_names(&function.decorator_list),
                    parametrized: is_parametrized(&function.decorator_list),
                    framework: if in_unittest_class {
                        "unittest"
                    } else {
                        "pytest"
                    },
                    assertions: collect_assertions_from_statements(&function.body, source),
                });
            }
            Stmt::AsyncFunctionDef(function) if function.name.as_str().starts_with("test_") => {
                out.push(PythonTest {
                    name: function.name.to_string(),
                    file: file.to_path_buf(),
                    line: line_for_range_start(source, function.range),
                    body_text: text_for_range(source, function.range),
                    imports: imports.to_vec(),
                    decorators: decorator_names(&function.decorator_list),
                    parametrized: is_parametrized(&function.decorator_list),
                    framework: if in_unittest_class {
                        "unittest"
                    } else {
                        "pytest"
                    },
                    assertions: collect_assertions_from_statements(&function.body, source),
                });
            }
            Stmt::ClassDef(class) => {
                collect_tests_from_statements(
                    file,
                    source,
                    &class.body,
                    is_unittest_class(class) || in_unittest_class,
                    imports,
                    out,
                );
            }
            _ => {}
        }
    }
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
    let (oracle_kind, oracle_strength) = oracle_for_assert_expr(assert_stmt.test.as_ref());
    PythonAssertion {
        text: text_for_range(source, assert_stmt.range).trim().to_string(),
        line: line_for_range_start(source, assert_stmt.range),
        oracle_kind,
        oracle_strength,
    }
}

fn assertion_from_expr(expr: &Expr, source: &str) -> Option<PythonAssertion> {
    let Expr::Call(call) = expr else {
        return None;
    };
    let (oracle_kind, oracle_strength) = oracle_for_call(call)?;
    Some(PythonAssertion {
        text: text_for_range(source, call.range).trim().to_string(),
        line: line_for_range_start(source, call.range),
        oracle_kind,
        oracle_strength,
    })
}

fn oracle_for_assert_expr(expr: &Expr) -> (OracleKind, OracleStrength) {
    match expr {
        Expr::Compare(compare) => oracle_for_compare_ops(&compare.ops),
        Expr::Call(call) => {
            if expr_full_name(call.func.as_ref()).is_some_and(|name| name == "isinstance") {
                (OracleKind::RelationalCheck, OracleStrength::Weak)
            } else {
                oracle_for_call(call).unwrap_or((OracleKind::SmokeOnly, OracleStrength::Smoke))
            }
        }
        _ => (OracleKind::SmokeOnly, OracleStrength::Smoke),
    }
}

fn oracle_for_compare_ops(ops: &[ast::CmpOp]) -> (OracleKind, OracleStrength) {
    if ops.iter().any(|op| matches!(op, ast::CmpOp::Eq)) {
        (OracleKind::ExactValue, OracleStrength::Strong)
    } else {
        (OracleKind::RelationalCheck, OracleStrength::Weak)
    }
}

fn oracle_for_call(call: &ast::ExprCall) -> Option<(OracleKind, OracleStrength)> {
    let name = expr_full_name(call.func.as_ref())?;
    let last_segment = name.rsplit('.').next().unwrap_or(name.as_str());
    match last_segment {
        "assertEqual" => Some((OracleKind::ExactValue, OracleStrength::Strong)),
        "assertNotEqual" => Some((OracleKind::RelationalCheck, OracleStrength::Weak)),
        "assertTrue" | "assertFalse" => Some((OracleKind::SmokeOnly, OracleStrength::Smoke)),
        "assertRaises" | "assertRaisesRegex" => {
            Some((OracleKind::BroadError, OracleStrength::Weak))
        }
        "raises" if name == "pytest.raises" => Some((OracleKind::BroadError, OracleStrength::Weak)),
        "assert_called"
        | "assert_called_once"
        | "assert_called_with"
        | "assert_called_once_with"
        | "assert_any_call"
        | "assert_has_calls"
        | "assert_not_called" => Some((OracleKind::MockExpectation, OracleStrength::Medium)),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PythonRelationKind {
    SyntacticCall,
    ImportAliasCall,
    SameStem,
}

impl PythonRelationKind {
    fn rank(self) -> u8 {
        match self {
            Self::SyntacticCall => 3,
            Self::ImportAliasCall => 2,
            Self::SameStem => 1,
        }
    }

    fn uses_oracle(self) -> bool {
        matches!(self, Self::SyntacticCall | Self::ImportAliasCall)
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::SyntacticCall => "syntactic_call",
            Self::ImportAliasCall => "import_alias_call",
            Self::SameStem => "same_stem",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PythonRelatedCandidate<'a> {
    test: &'a PythonTest,
    relation: PythonRelationKind,
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
    None
}

fn body_calls_owner(body_text: &str, owner: &PythonOwner) -> bool {
    let direct_call = format!("{}(", owner.name);
    let method_call = format!(".{}(", owner.name);
    let qualified_call = format!("{}(", owner.qualified_name);
    body_text.contains(&direct_call)
        || body_text.contains(&method_call)
        || body_text.contains(&qualified_call)
}

fn import_alias_calls_owner(test: &PythonTest, owner: &PythonOwner) -> bool {
    test.imports.iter().any(|import| {
        import.imported == owner.name
            && import.alias != owner.name
            && test.body_text.contains(&format!("{}(", import.alias))
    })
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

fn classify_change(
    file: &Path,
    line: usize,
    line_text: &str,
    owners: &[PythonOwner],
    all_tests: &[PythonTest],
) -> Option<Finding> {
    let changed_file = normalized_path(file);
    let owner = owners
        .iter()
        .filter(|owner| normalized_path(&owner.file) == changed_file)
        .find(|owner| line >= owner.start_line && line <= owner.end_line)?;
    let related_candidates = related_test_candidates(owner, all_tests);
    let related = find_related_tests(owner, all_tests);
    let static_limit = static_limit_for_change(line_text, owner, &related_candidates);
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

    let (class, reach_state, observe_state, discriminate_state, mut missing) = if related.is_empty()
    {
        (
            ExposureClass::NoStaticPath,
            StageState::No,
            StageState::No,
            StageState::No,
            vec![format!(
                "No Python test references `{}(`; add a pytest or unittest test that calls the changed owner.",
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
            vec![format!(
                "Related Python test reaches `{}` but the strongest extracted oracle is `{}`; add or verify an exact-value assertion to make the preview finding stronger.",
                owner.name,
                strongest_kind.as_str()
            )],
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
    let (family, delta) = classify_probe_shape(line_text);
    let probe = Probe {
        id: ProbeId(format!("probe:{id_path}:{line}:python_preview")),
        location: SourceLocation::new(file.to_string_lossy().as_ref(), line, 1),
        owner: None,
        family,
        delta,
        before: None,
        after: Some(line_text.to_string()),
        expression: line_text.to_string(),
        expected_sinks: Vec::new(),
        required_oracles: Vec::new(),
    };

    let related_count = related.len();
    let reach_summary = format!(
        "{} related Python test(s) found for owner `{}`",
        related_count, owner.name
    );
    let reach = StageEvidence::new(reach_state, Confidence::Low, &reach_summary);
    let infect = StageEvidence::new(
        StageState::Unknown,
        Confidence::Low,
        "Python preview adapter does not yet model infection.",
    );
    let propagate = StageEvidence::new(
        StageState::Unknown,
        Confidence::Low,
        "Python preview adapter does not yet model propagation.",
    );
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
        "Python preview adapter found no strong discriminator; use `assert ... == ...` or `self.assertEqual(...)` to escalate.".to_string()
    };
    let discriminate =
        StageEvidence::new(discriminate_state, Confidence::Low, discriminate_summary);

    let recommended = match class {
        ExposureClass::Exposed => {
            "Python preview: changed behavior is observed under a strong oracle; verify the assertion targets the changed boundary value.".to_string()
        }
        ExposureClass::NoStaticPath => {
            "Python preview: no related test calls the changed owner; add a pytest or unittest test that exercises this behavior.".to_string()
        }
        _ => {
            "Python preview: add or verify a focused exact-value assertion (`assert ... == ...` or `self.assertEqual(...)`) for the changed behavior.".to_string()
        }
    };
    let confidence_value = if matches!(class, ExposureClass::Exposed) {
        0.6
    } else {
        0.4
    };

    let mut evidence = vec![
        format!("owner: {}", owner.qualified_name),
        format!("owner_kind: {}", owner.owner_kind.as_str()),
    ];
    if !owner.decorators.is_empty() {
        evidence.push(format!("owner_decorators: {}", owner.decorators.join(", ")));
    }
    if let Some(limit) = &static_limit {
        evidence.push(limit.evidence.clone());
    }
    for candidate in related_candidates {
        let test = candidate.test;
        evidence.push(format!(
            "test_framework: {} ({})",
            test.framework, test.name
        ));
        evidence.push(format!(
            "related_test_relation: {} ({})",
            candidate.relation.as_str(),
            test.name
        ));
        if candidate.relation.uses_oracle()
            && let Some(assertion) = strongest_assertion(&test.assertions)
        {
            evidence.push(format!(
                "test_oracle: {} {} ({})",
                assertion.oracle_kind.as_str(),
                assertion.oracle_strength.as_str(),
                test.name
            ));
        }
    }

    Some(Finding {
        id: probe.id.0.clone(),
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
        flow_sinks: Vec::new(),
        activation: Default::default(),
        stop_reasons: Vec::new(),
        related_tests: related,
        recommended_next_step: Some(recommended),
        language: Some(DomainLanguageId::Python),
        language_status: Some(LanguageStatus::Preview),
        owner_kind: Some(owner.owner_kind),
        static_limit_kind: static_limit.map(|limit| limit.kind),
    })
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
        if name == ".git"
            || name == "target"
            || name == "node_modules"
            || name == ".ripr"
            || name == ".direnv"
            || name == "__pycache__"
            || name == ".venv"
            || name == "venv"
            || name == "env"
            || name == ".mypy_cache"
        {
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
            if adapter.accepts_path(&path) {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                out.push(relative);
            }
        }
    }
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
            if is_test_file(relative) {
                all_tests.extend(extract_tests(relative, &source));
            } else {
                all_owners.extend(extract_owners(relative, &source));
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
                "Policy.from_config"
            ]
        );
        assert_eq!(owners[0].owner_kind, OwnerKind::Function);
        assert_eq!(owners[1].decorators, vec!["async_def"]);
        assert_eq!(owners[2].owner_kind, OwnerKind::Method);
        assert_eq!(owners[3].owner_kind, OwnerKind::ClassMethod);
        assert_eq!(owners[4].owner_kind, OwnerKind::ClassMethod);
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
            vec!["test_apply_discount", "test_apply_method"]
        );
        assert!(tests[0].parametrized);
        assert_eq!(tests[0].framework, "pytest");
        assert_eq!(tests[1].framework, "unittest");
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
        Ok(())
    }

    #[test]
    fn classify_change_returns_weakly_exposed_when_related_test_exists() -> Result<(), String> {
        let owners = extract_owners(
            Path::new("src/pricing.py"),
            "def apply_discount(amount):\n    if amount >= 100:\n        return amount - 10\n    return amount\n",
        );
        let tests = extract_tests(
            Path::new("tests/test_pricing.py"),
            "def test_apply_discount():\n    result = apply_discount(100)\n",
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

        assert_eq!(finding.class, ExposureClass::WeaklyExposed);
        assert_eq!(finding.language, Some(DomainLanguageId::Python));
        assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
        assert_eq!(finding.owner_kind, Some(OwnerKind::Function));
        assert_eq!(finding.related_tests.len(), 1);
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
    fn classify_change_surfaces_static_limit_without_downgrading_strong_evidence()
    -> Result<(), String> {
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

        assert_eq!(finding.class, ExposureClass::Exposed);
        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::DynamicDispatch)
        );
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
            "def test_docs_mentions_owner():\n    assert \"apply_discount\" in \"apply_discount\"\n",
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
