//! TypeScript preview adapter — owner + test sub-slice.
//!
//! See `docs/specs/RIPR-SPEC-0027-typescript-preview-static-facts.md` and
//! `docs/adr/0008-typescript-parser-substrate.md`.
//!
//! This sub-slice extracts syntax-first owner facts and Jest/Vitest
//! `test(...)` / `it(...)` / `.each(...)` blocks from TypeScript /
//! JavaScript files,
//! matches related tests by name reference, and emits one preview-tagged
//! `Finding` per changed line that falls within an owner. The classifier
//! is intentionally minimal — it produces a two-way gradient:
//!
//! - `WeaklyExposed` when the changed-line's owner is referenced by at
//!   least one test (any oracle, including unknown).
//! - `NoStaticPath` when no related test references the owner.
//!
//! Assertion-shape extraction (refining `WeaklyExposed` → `Exposed` for
//! exact-value oracles), probe-family shape detection, and explicit
//! static-limit reporting land in follow-up issues:
//! #767 (assertion shapes), #768 (probe shapes), #769 (static limits).
//! Nested `describe(...)` blocks are syntax-walked for test discovery, but
//! related-test matching remains intentionally bounded to owner-call tokens.

use super::super::{AnalysisOptions, diff::ChangedFile};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::config::OraclePolicy;
use crate::domain::{
    Confidence, DeltaKind, ExposureClass, Finding, LanguageId as DomainLanguageId, LanguageStatus,
    OracleKind, OracleStrength, OwnerKind, Probe, ProbeFamily, ProbeId, RelatedTest,
    RevealEvidence, RiprEvidence, SourceLocation, StageEvidence, StageState, StaticLimitKind,
    StopReason, SymbolId,
};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ArrowFunctionExpression, BindingPattern, Class, ClassElement, Declaration,
    ExportDefaultDeclarationKind, Expression, Function, ImportDeclarationSpecifier,
    ImportOrExportKind, MethodDefinition, ModuleExportName, PropertyKey, Statement,
    VariableDeclaration, VariableDeclarator,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use std::path::{Path, PathBuf};

/// TypeScript / JavaScript preview adapter.
///
/// Stateless: routing, parsing, and per-file extraction only.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TypeScriptAdapter;

fn source_type_for(path: &Path) -> SourceType {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("tsx") => SourceType::tsx(),
        Some("ts") => SourceType::ts(),
        Some("jsx") => SourceType::jsx(),
        Some("js") => SourceType::mjs(),
        _ => SourceType::mjs(),
    }
}

/// Owner extracted from a TypeScript / JavaScript source file.
///
/// Covers the syntax-first owner kinds accepted for the preview surface.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptOwner {
    name: String,
    file: PathBuf,
    start_line: usize,
    end_line: usize,
    owner_kind: OwnerKind,
}

impl TypeScriptOwner {
    fn symbol_id(&self) -> SymbolId {
        SymbolId(format!(
            "{}:{}::{}",
            output_language_for(&self.file).as_str(),
            normalized_path(&self.file),
            self.name
        ))
    }
}

/// Test block extracted from a TypeScript / JavaScript test file.
///
/// Covers syntax-first Jest/Vitest `test('name', fn)`, `it('name', fn)`,
/// and array-form `.each(...)('name', fn)` expression statements, including
/// nested `describe(...)` blocks.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptTest {
    name: String,
    file: PathBuf,
    line: usize,
    body_text: String,
    assertions: Vec<TypeScriptAssertion>,
    /// Module paths referenced by syntactic `vi.mock("...")` /
    /// `jest.mock("...")` calls discovered at the top level of the
    /// containing test file. Populated once per file and cloned into
    /// every `TypeScriptTest` parsed from that file so the classifier
    /// can surface the `mocked_module` static-limit without re-parsing.
    /// Empty when no syntactic mock indirection is present.
    mocks_in_file: Vec<String>,
    /// Runtime imports discovered at the top level of the containing test
    /// file. Used only to map relative named or namespace imports back to a
    /// source owner before considering alias calls related.
    imports_in_file: Vec<TypeScriptImport>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptImport {
    source: String,
    imported: Option<String>,
    local: String,
    namespace: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptParseLimit {
    file: PathBuf,
    reason: String,
}

/// Assertion shape extracted from a single `expect(actual).matcher(...)`
/// chain inside a test body.
///
/// `matcher` is the canonical matcher name (`toBe`, `toEqual`, `toThrow`,
/// `toMatchSnapshot`, `toHaveBeenCalledWith`, ...). The full Jest/Vitest
/// matcher surface is large; this preview slice maps the most common
/// matchers to oracle vocabulary and tags the rest as `Unknown`.
/// Async-aware (`.resolves` / `.rejects`) chains are recognised by syntax;
/// custom matchers stay `Unknown`.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptAssertion {
    matcher: String,
    argument_count: usize,
    line: usize,
    oracle_kind: OracleKind,
    oracle_strength: OracleStrength,
}

fn oracle_for_matcher(matcher: &str) -> (OracleKind, OracleStrength) {
    match matcher {
        "toBe" | "toEqual" | "toStrictEqual" => (OracleKind::ExactValue, OracleStrength::Strong),
        "toThrow" | "toThrowError" => (OracleKind::BroadError, OracleStrength::Weak),
        "toMatchSnapshot" | "toMatchInlineSnapshot" => {
            (OracleKind::Snapshot, OracleStrength::Medium)
        }
        "toHaveBeenCalled"
        | "toHaveBeenCalledWith"
        | "toHaveBeenCalledTimes"
        | "toHaveBeenLastCalledWith"
        | "toHaveBeenNthCalledWith" => (OracleKind::MockExpectation, OracleStrength::Medium),
        "toBeTruthy" | "toBeFalsy" | "toBeDefined" | "toBeUndefined" | "toBeNull" | "toBeNaN" => {
            (OracleKind::SmokeOnly, OracleStrength::Smoke)
        }
        "toContain"
        | "toMatch"
        | "toBeGreaterThan"
        | "toBeGreaterThanOrEqual"
        | "toBeLessThan"
        | "toBeLessThanOrEqual"
        | "toHaveLength"
        | "toHaveProperty" => (OracleKind::RelationalCheck, OracleStrength::Weak),
        _ => (OracleKind::Unknown, OracleStrength::Unknown),
    }
}

/// Whether a path is a test file by convention (`*.test.ts`, `*.spec.ts`,
/// and `.tsx` / `.js` / `.jsx` variants).
fn is_test_file(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let stem_extensions: &[&str] = &[
        ".test.ts",
        ".test.tsx",
        ".test.js",
        ".test.jsx",
        ".spec.ts",
        ".spec.tsx",
        ".spec.js",
        ".spec.jsx",
    ];
    stem_extensions
        .iter()
        .any(|suffix| file_name.ends_with(suffix))
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

fn normalized_path(path: &Path) -> String {
    let mut normalized = path.to_string_lossy().replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_string();
    }
    normalized
}

fn extract_owners(file: &Path, source: &str) -> Vec<TypeScriptOwner> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, source_type_for(file)).parse();
    if !ret.errors.is_empty() {
        return Vec::new();
    }
    let mut owners = Vec::new();
    for stmt in &ret.program.body {
        owners.extend(owners_from_statement(stmt, file, source));
    }
    owners
}

fn parse_error_reason(file: &Path, source: &str) -> Option<String> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, source_type_for(file)).parse();
    if ret.errors.is_empty() {
        None
    } else {
        Some(format!("{} parser error(s)", ret.errors.len()))
    }
}

fn owners_from_statement(stmt: &Statement<'_>, file: &Path, source: &str) -> Vec<TypeScriptOwner> {
    if let Statement::FunctionDeclaration(func) = stmt
        && let Some(id) = &func.id
    {
        return vec![owner_from_function(
            file,
            source,
            id.name.as_str(),
            func,
            function_owner_kind(
                file,
                source,
                id.name.as_str(),
                func.span.start,
                func.span.end,
            ),
        )];
    }
    if let Statement::ExportNamedDeclaration(export) = stmt
        && let Some(decl) = export.declaration.as_ref()
    {
        return owners_from_declaration(decl, file, source);
    }
    if let Statement::ExportDefaultDeclaration(export) = stmt {
        return owners_from_default_export(&export.declaration, file, source);
    }
    owners_from_statement_declaration(stmt, file, source)
}

fn owners_from_statement_declaration(
    stmt: &Statement<'_>,
    file: &Path,
    source: &str,
) -> Vec<TypeScriptOwner> {
    match stmt {
        Statement::VariableDeclaration(decl) => {
            owners_from_variable_declaration(decl, file, source)
        }
        Statement::ClassDeclaration(class) => owners_from_class(class, file, source),
        _ => Vec::new(),
    }
}

fn owners_from_declaration(
    decl: &Declaration<'_>,
    file: &Path,
    source: &str,
) -> Vec<TypeScriptOwner> {
    match decl {
        Declaration::FunctionDeclaration(func) => func
            .id
            .as_ref()
            .map(|id| {
                vec![owner_from_function(
                    file,
                    source,
                    id.name.as_str(),
                    func,
                    function_owner_kind(
                        file,
                        source,
                        id.name.as_str(),
                        func.span.start,
                        func.span.end,
                    ),
                )]
            })
            .unwrap_or_default(),
        Declaration::VariableDeclaration(decl) => {
            owners_from_variable_declaration(decl, file, source)
        }
        Declaration::ClassDeclaration(class) => owners_from_class(class, file, source),
        _ => Vec::new(),
    }
}

fn owners_from_default_export(
    decl: &ExportDefaultDeclarationKind<'_>,
    file: &Path,
    source: &str,
) -> Vec<TypeScriptOwner> {
    match decl {
        ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
            let name = func
                .id
                .as_ref()
                .map(|id| id.name.as_str())
                .unwrap_or("default");
            vec![owner_from_function(
                file,
                source,
                name,
                func,
                function_owner_kind(file, source, name, func.span.start, func.span.end),
            )]
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            owners_from_class(class, file, source)
        }
        ExportDefaultDeclarationKind::ArrowFunctionExpression(arrow) => vec![owner_from_arrow(
            file,
            source,
            "default",
            arrow,
            arrow.span.start,
        )],
        _ => Vec::new(),
    }
}

fn owners_from_variable_declaration(
    decl: &VariableDeclaration<'_>,
    file: &Path,
    source: &str,
) -> Vec<TypeScriptOwner> {
    decl.declarations
        .iter()
        .filter_map(|declarator| owner_from_variable_declarator(declarator, file, source))
        .collect()
}

fn owner_from_variable_declarator(
    declarator: &VariableDeclarator<'_>,
    file: &Path,
    source: &str,
) -> Option<TypeScriptOwner> {
    let name = binding_identifier_name(&declarator.id)?;
    let init = declarator.init.as_ref()?;
    match init {
        Expression::ArrowFunctionExpression(arrow) => Some(owner_from_arrow(
            file,
            source,
            name,
            arrow,
            declarator.span.start,
        )),
        Expression::FunctionExpression(func) => Some(owner_from_function(
            file,
            source,
            name,
            func,
            function_owner_kind(file, source, name, func.span.start, func.span.end),
        )),
        _ => Some(TypeScriptOwner {
            name: name.to_string(),
            file: file.to_path_buf(),
            start_line: line_for_offset(source, declarator.span.start as usize),
            end_line: line_for_offset(source, declarator.span.end as usize),
            owner_kind: OwnerKind::ModuleFunction,
        }),
    }
}

fn owner_from_function(
    file: &Path,
    source: &str,
    name: &str,
    func: &Function<'_>,
    owner_kind: OwnerKind,
) -> TypeScriptOwner {
    TypeScriptOwner {
        name: name.to_string(),
        file: file.to_path_buf(),
        start_line: line_for_offset(source, func.span.start as usize),
        end_line: line_for_offset(source, func.span.end as usize),
        owner_kind,
    }
}

fn owner_from_arrow(
    file: &Path,
    source: &str,
    name: &str,
    arrow: &ArrowFunctionExpression<'_>,
    owner_start: u32,
) -> TypeScriptOwner {
    TypeScriptOwner {
        name: name.to_string(),
        file: file.to_path_buf(),
        start_line: line_for_offset(source, owner_start as usize),
        end_line: line_for_offset(source, arrow.span.end as usize),
        owner_kind: arrow_owner_kind(file, source, name, arrow.span.start, arrow.span.end),
    }
}

fn owners_from_class(class: &Class<'_>, file: &Path, source: &str) -> Vec<TypeScriptOwner> {
    let mut owners = Vec::new();
    for element in &class.body.body {
        if let ClassElement::MethodDefinition(method) = element
            && let Some(owner) = owner_from_method(method, file, source)
        {
            owners.push(owner);
        }
    }
    owners
}

fn owner_from_method(
    method: &MethodDefinition<'_>,
    file: &Path,
    source: &str,
) -> Option<TypeScriptOwner> {
    if method.computed {
        return None;
    }
    let name = property_key_name(&method.key)?;
    Some(TypeScriptOwner {
        name,
        file: file.to_path_buf(),
        start_line: line_for_offset(source, method.span.start as usize),
        end_line: line_for_offset(source, method.span.end as usize),
        owner_kind: if method.r#static {
            OwnerKind::ClassMethod
        } else {
            OwnerKind::Method
        },
    })
}

fn binding_identifier_name<'a>(pattern: &'a BindingPattern<'a>) -> Option<&'a str> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        _ => None,
    }
}

fn function_owner_kind(file: &Path, source: &str, name: &str, start: u32, end: u32) -> OwnerKind {
    if looks_like_component_owner(file, source, name, start, end) {
        OwnerKind::Component
    } else {
        OwnerKind::Function
    }
}

fn arrow_owner_kind(file: &Path, source: &str, name: &str, start: u32, end: u32) -> OwnerKind {
    if looks_like_component_owner(file, source, name, start, end) {
        OwnerKind::Component
    } else {
        OwnerKind::ArrowFunction
    }
}

fn looks_like_component_owner(file: &Path, source: &str, name: &str, start: u32, end: u32) -> bool {
    if !matches!(
        file.extension().and_then(|extension| extension.to_str()),
        Some("tsx" | "jsx")
    ) || !starts_with_uppercase(name)
    {
        return false;
    }
    let start = start as usize;
    let end = end as usize;
    let Some(slice) = source.get(start..end) else {
        return false;
    };
    contains_jsx_like_return(slice)
}

fn starts_with_uppercase(name: &str) -> bool {
    name.chars().next().is_some_and(|ch| ch.is_uppercase())
}

fn contains_jsx_like_return(slice: &str) -> bool {
    slice.contains("return <")
        || slice.contains("=> <")
        || slice
            .split("return (")
            .skip(1)
            .any(|tail| tail.trim_start().starts_with('<'))
        || slice
            .split("=> (")
            .skip(1)
            .any(|tail| tail.trim_start().starts_with('<'))
}

fn extract_tests(file: &Path, source: &str) -> Vec<TypeScriptTest> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, source_type_for(file)).parse();
    if !ret.errors.is_empty() {
        return Vec::new();
    }
    let imports = extract_imports_from_statements(&ret.program.body);
    let mocks = extract_mocks_from_statements(&ret.program.body);
    let mut tests = Vec::new();
    collect_tests_from_statements(
        &ret.program.body,
        file,
        source,
        &mocks,
        &imports,
        &mut Vec::new(),
        &mut tests,
    );
    tests
}

fn extract_imports_from_statements(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
) -> Vec<TypeScriptImport> {
    let mut out: Vec<TypeScriptImport> = Vec::new();
    for stmt in statements {
        let Statement::ImportDeclaration(import) = stmt else {
            continue;
        };
        if import.import_kind == ImportOrExportKind::Type {
            continue;
        }
        let source = import.source.value.to_string();
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        for specifier in specifiers {
            match specifier {
                ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                    if specifier.import_kind == ImportOrExportKind::Type {
                        continue;
                    }
                    let Some(imported) = module_export_name_text(&specifier.imported) else {
                        continue;
                    };
                    push_unique_import(
                        &mut out,
                        TypeScriptImport {
                            source: source.clone(),
                            imported: Some(imported),
                            local: specifier.local.name.as_str().to_string(),
                            namespace: false,
                        },
                    );
                }
                ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => {
                    push_unique_import(
                        &mut out,
                        TypeScriptImport {
                            source: source.clone(),
                            imported: Some("default".to_string()),
                            local: specifier.local.name.as_str().to_string(),
                            namespace: false,
                        },
                    );
                }
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                    push_unique_import(
                        &mut out,
                        TypeScriptImport {
                            source: source.clone(),
                            imported: None,
                            local: specifier.local.name.as_str().to_string(),
                            namespace: true,
                        },
                    );
                }
            }
        }
    }
    out
}

fn push_unique_import(out: &mut Vec<TypeScriptImport>, import: TypeScriptImport) {
    if !out.iter().any(|existing| existing == &import) {
        out.push(import);
    }
}

fn module_export_name_text(name: &ModuleExportName<'_>) -> Option<String> {
    match name {
        ModuleExportName::IdentifierName(ident) => Some(ident.name.as_str().to_string()),
        ModuleExportName::IdentifierReference(ident) => Some(ident.name.as_str().to_string()),
        ModuleExportName::StringLiteral(literal) => Some(literal.value.to_string()),
    }
}

/// Walk a list of top-level statements and collect every syntactic
/// `vi.mock("path")` / `jest.mock("path")` argument we see. The list is
/// deduplicated and used by the classifier to surface the
/// `mocked_module` static-limit per RIPR-SPEC-0026.
///
/// This is purely syntactic — the adapter does not resolve the mocked
/// module identifier through the project's import graph, so the limit
/// surfaces exactly when the test file contains the mock call shape.
fn extract_mocks_from_statements(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for stmt in statements {
        let Statement::ExpressionStatement(expr_stmt) = stmt else {
            continue;
        };
        let Expression::CallExpression(call) = &expr_stmt.expression else {
            continue;
        };
        let Expression::StaticMemberExpression(member) = &call.callee else {
            continue;
        };
        let Expression::Identifier(object_ident) = &member.object else {
            continue;
        };
        let object_name = object_ident.name.as_str();
        if object_name != "vi" && object_name != "jest" {
            continue;
        }
        if member.property.name.as_str() != "mock" {
            continue;
        }
        let Some(first_arg) = call.arguments.first() else {
            continue;
        };
        let oxc_ast::ast::Argument::StringLiteral(literal) = first_arg else {
            continue;
        };
        let path = literal.value.to_string();
        if !out.iter().any(|existing| existing == &path) {
            out.push(path);
        }
    }
    out
}

fn collect_tests_from_statements(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
    file: &Path,
    source: &str,
    mocks: &[String],
    imports: &[TypeScriptImport],
    describe_stack: &mut Vec<String>,
    tests: &mut Vec<TypeScriptTest>,
) {
    for stmt in statements {
        if let Some((describe_name, body)) = describe_body_from_statement(stmt) {
            describe_stack.push(describe_name);
            collect_tests_from_statements(
                body,
                file,
                source,
                mocks,
                imports,
                describe_stack,
                tests,
            );
            describe_stack.pop();
            continue;
        }
        if let Some(mut test) = test_from_statement(stmt, file, source, describe_stack) {
            test.mocks_in_file = mocks.to_vec();
            test.imports_in_file = imports.to_vec();
            tests.push(test);
        }
    }
}

fn describe_body_from_statement<'a>(
    stmt: &'a Statement<'a>,
) -> Option<(String, &'a oxc_allocator::Vec<'a, Statement<'a>>)> {
    let Statement::ExpressionStatement(expr_stmt) = stmt else {
        return None;
    };
    let Expression::CallExpression(call) = &expr_stmt.expression else {
        return None;
    };
    let Expression::Identifier(ident) = &call.callee else {
        return None;
    };
    if ident.name.as_str() != "describe" {
        return None;
    }
    let name = string_argument(call.arguments.first()?)?;
    let body = function_body_statements_from_argument(call.arguments.get(1)?)?;
    Some((name, body))
}

fn test_from_statement(
    stmt: &Statement<'_>,
    file: &Path,
    source: &str,
    describe_stack: &[String],
) -> Option<TypeScriptTest> {
    let Statement::ExpressionStatement(expr_stmt) = stmt else {
        return None;
    };
    let Expression::CallExpression(call) = &expr_stmt.expression else {
        return None;
    };
    let (name, assertions) = test_name_and_assertions_from_call(call, source)?;
    Some(TypeScriptTest {
        name: qualified_test_name(describe_stack, &name),
        file: file.to_path_buf(),
        line: line_for_offset(source, call.span.start as usize),
        body_text: source[call.span.start as usize..call.span.end as usize].to_string(),
        assertions,
        // Populated by `extract_tests` (the only public extractor) once
        // per file before the test is returned to the caller.
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
    })
}

fn test_name_and_assertions_from_call(
    call: &oxc_ast::ast::CallExpression<'_>,
    source: &str,
) -> Option<(String, Vec<TypeScriptAssertion>)> {
    if test_callee_is_identifier(call) {
        let name = string_argument(call.arguments.first()?)?;
        let assertions = function_body_statements_from_argument(call.arguments.get(1)?)
            .map(|statements| collect_expect_assertions_in_statements(statements, source))
            .unwrap_or_default();
        return Some((name, assertions));
    }

    if test_callee_is_each(call) {
        let name = string_argument(call.arguments.first()?)?;
        let assertions = function_body_statements_from_argument(call.arguments.get(1)?)
            .map(|statements| collect_expect_assertions_in_statements(statements, source))
            .unwrap_or_default();
        return Some((name, assertions));
    }

    None
}

fn test_callee_is_identifier(call: &oxc_ast::ast::CallExpression<'_>) -> bool {
    let Expression::Identifier(ident) = &call.callee else {
        return false;
    };
    matches!(ident.name.as_str(), "test" | "it")
}

fn test_callee_is_each(call: &oxc_ast::ast::CallExpression<'_>) -> bool {
    let Expression::CallExpression(each_call) = &call.callee else {
        return false;
    };
    let Expression::StaticMemberExpression(member) = &each_call.callee else {
        return false;
    };
    if member.property.name.as_str() != "each" {
        return false;
    }
    let Expression::Identifier(ident) = &member.object else {
        return false;
    };
    matches!(ident.name.as_str(), "test" | "it")
}

fn string_argument(arg: &oxc_ast::ast::Argument<'_>) -> Option<String> {
    match arg {
        oxc_ast::ast::Argument::StringLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

fn function_body_statements_from_argument<'a>(
    arg: &'a oxc_ast::ast::Argument<'a>,
) -> Option<&'a oxc_allocator::Vec<'a, Statement<'a>>> {
    match arg {
        oxc_ast::ast::Argument::ArrowFunctionExpression(arrow) => Some(&arrow.body.statements),
        oxc_ast::ast::Argument::FunctionExpression(func) => {
            func.body.as_ref().map(|body| &body.statements)
        }
        _ => None,
    }
}

fn qualified_test_name(describe_stack: &[String], name: &str) -> String {
    if describe_stack.is_empty() {
        return name.to_string();
    }
    let mut parts = describe_stack.to_vec();
    parts.push(name.to_string());
    parts.join(" ")
}

/// Walk a list of statements (e.g., a function body) and collect every
/// `expect(actual).matcher(...)` expression statement we recognise. Test
/// discriminators are often guarded by setup branches or cleanup blocks, so
/// this recurses through common control-flow bodies while still staying
/// syntax-only and conservative.
fn collect_expect_assertions_in_statements(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
    source: &str,
) -> Vec<TypeScriptAssertion> {
    let mut out = Vec::new();
    for stmt in statements {
        collect_expect_assertions_in_statement(stmt, source, &mut out);
    }
    out
}

fn collect_expect_assertions_in_statement(
    stmt: &Statement<'_>,
    source: &str,
    out: &mut Vec<TypeScriptAssertion>,
) {
    match stmt {
        Statement::BlockStatement(block) => {
            collect_expect_assertions_from_statement_vec(&block.body, source, out);
        }
        Statement::ExpressionStatement(expr_stmt) => {
            if let Some(assertion) = expect_assertion_from_expression(&expr_stmt.expression, source)
            {
                out.push(assertion);
            }
        }
        Statement::ReturnStatement(return_stmt) => {
            if let Some(argument) = &return_stmt.argument
                && let Some(assertion) = expect_assertion_from_expression(argument, source)
            {
                out.push(assertion);
            }
        }
        Statement::IfStatement(if_stmt) => {
            collect_expect_assertions_in_statement(&if_stmt.consequent, source, out);
            if let Some(alternate) = &if_stmt.alternate {
                collect_expect_assertions_in_statement(alternate, source, out);
            }
        }
        Statement::DoWhileStatement(do_while) => {
            collect_expect_assertions_in_statement(&do_while.body, source, out);
        }
        Statement::WhileStatement(while_stmt) => {
            collect_expect_assertions_in_statement(&while_stmt.body, source, out);
        }
        Statement::ForStatement(for_stmt) => {
            collect_expect_assertions_in_statement(&for_stmt.body, source, out);
        }
        Statement::ForInStatement(for_in) => {
            collect_expect_assertions_in_statement(&for_in.body, source, out);
        }
        Statement::ForOfStatement(for_of) => {
            collect_expect_assertions_in_statement(&for_of.body, source, out);
        }
        Statement::LabeledStatement(labeled) => {
            collect_expect_assertions_in_statement(&labeled.body, source, out);
        }
        Statement::SwitchStatement(switch_stmt) => {
            for case in &switch_stmt.cases {
                collect_expect_assertions_from_statement_vec(&case.consequent, source, out);
            }
        }
        Statement::TryStatement(try_stmt) => {
            collect_expect_assertions_from_statement_vec(&try_stmt.block.body, source, out);
            if let Some(handler) = &try_stmt.handler {
                collect_expect_assertions_from_statement_vec(&handler.body.body, source, out);
            }
            if let Some(finalizer) = &try_stmt.finalizer {
                collect_expect_assertions_from_statement_vec(&finalizer.body, source, out);
            }
        }
        Statement::WithStatement(with_stmt) => {
            collect_expect_assertions_in_statement(&with_stmt.body, source, out);
        }
        _ => {}
    }
}

fn collect_expect_assertions_from_statement_vec(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
    source: &str,
    out: &mut Vec<TypeScriptAssertion>,
) {
    for stmt in statements {
        collect_expect_assertions_in_statement(stmt, source, out);
    }
}

/// Match the simplest `expect(actual).matcher(...)` shape on a top-level
/// expression. Async-aware `.resolves.matcher` / `.rejects.matcher`
/// chains are recognised by checking for one extra member-access hop
/// before the inner `expect(...)` call; the matcher remains the final
/// property name.
fn expect_assertion_from_expression(
    expr: &Expression<'_>,
    source: &str,
) -> Option<TypeScriptAssertion> {
    let expr = match expr {
        Expression::AwaitExpression(await_expr) => &await_expr.argument,
        _ => expr,
    };
    let Expression::CallExpression(outer_call) = expr else {
        return None;
    };
    let Expression::StaticMemberExpression(outer_member) = &outer_call.callee else {
        return None;
    };
    let matcher = outer_member.property.name.as_str();

    // Inner shape is either `expect(...)` directly or an
    // `expect(...).resolves` / `.rejects` chain.
    let inner = &outer_member.object;
    let inner_is_expect_call = match inner {
        // Direct: expect(...).matcher(...)
        Expression::CallExpression(inner_call) => {
            matches!(
                &inner_call.callee,
                Expression::Identifier(ident) if ident.name.as_str() == "expect"
            )
        }
        // Async chain: expect(...).resolves.matcher(...) etc.
        Expression::StaticMemberExpression(inner_member) => {
            let modifier = inner_member.property.name.as_str();
            if modifier != "resolves" && modifier != "rejects" {
                return None;
            }
            matches!(
                &inner_member.object,
                Expression::CallExpression(inner_call)
                    if matches!(&inner_call.callee, Expression::Identifier(ident) if ident.name.as_str() == "expect")
            )
        }
        _ => false,
    };
    if !inner_is_expect_call {
        return None;
    }

    let (oracle_kind, oracle_strength) = oracle_for_matcher(matcher);
    Some(TypeScriptAssertion {
        matcher: matcher.to_string(),
        argument_count: outer_call.arguments.len(),
        line: line_for_offset(source, outer_call.span.start as usize),
        oracle_kind,
        oracle_strength,
    })
}

fn find_related_tests(owner: &TypeScriptOwner, all_tests: &[TypeScriptTest]) -> Vec<RelatedTest> {
    all_tests
        .iter()
        .filter(|test| test_references_owner(test, owner))
        .map(|test| {
            let strongest = strongest_assertion(&test.assertions);
            let (oracle_kind, oracle_strength, oracle_text) = match strongest {
                Some(assertion) => (
                    assertion.oracle_kind.clone(),
                    assertion.oracle_strength.clone(),
                    Some(assertion_oracle_text(assertion)),
                ),
                None => (OracleKind::Unknown, OracleStrength::Unknown, None),
            };
            RelatedTest {
                name: test.name.clone(),
                file: test.file.clone(),
                line: test.line,
                oracle: oracle_text,
                oracle_kind,
                oracle_strength,
            }
        })
        .collect()
}

fn test_references_owner(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    if contains_call_name(&test.body_text, &owner.name)
        && !owner_name_shadowed_by_unrelated_import(test, owner)
    {
        return true;
    }
    test.imports_in_file.iter().any(|import| {
        import_source_matches_owner(import, &test.file, owner)
            && import_references_owner_call(import, &test.body_text, owner)
    })
}

fn contains_call_name(body_text: &str, call_name: &str) -> bool {
    let needle = format!("{call_name}(");
    body_text.match_indices(&needle).any(|(idx, _)| {
        has_call_boundary(body_text, idx)
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
            && !inside_block_comment(body_text, idx)
    })
}

fn has_call_boundary(body_text: &str, idx: usize) -> bool {
    body_text[..idx]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_javascript_identifier_char(ch) && ch != '.')
}

fn owner_name_shadowed_by_unrelated_import(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    test.imports_in_file
        .iter()
        .filter(|import| import.local == owner.name)
        .any(|import| {
            import.namespace
                || !import_source_matches_owner(import, &test.file, owner)
                || import.imported.as_deref().is_some_and(|imported| {
                    imported != owner.name.as_str() && imported != "default"
                })
        })
}

fn import_references_owner_call(
    import: &TypeScriptImport,
    body_text: &str,
    owner: &TypeScriptOwner,
) -> bool {
    if import.namespace {
        return contains_member_call_name(body_text, &import.local, &owner.name);
    }
    import.imported.as_deref() == Some(owner.name.as_str())
        && contains_call_name(body_text, &import.local)
}

fn import_source_matches_owner(
    import: &TypeScriptImport,
    test_file: &Path,
    owner: &TypeScriptOwner,
) -> bool {
    normalized_relative_import_module(test_file, &import.source)
        .is_some_and(|module| module == normalized_module_path(&owner.file))
}

fn normalized_relative_import_module(test_file: &Path, source: &str) -> Option<String> {
    if !source.starts_with("./") && !source.starts_with("../") {
        return None;
    }
    let mut parts = normalized_path(test_file.parent().unwrap_or_else(|| Path::new("")))
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let normalized_source = source.replace('\\', "/");
    for part in normalized_source.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part.to_string()),
        }
    }
    Some(strip_typescript_module_extension(&parts.join("/")))
}

fn normalized_module_path(path: &Path) -> String {
    strip_typescript_module_extension(&normalized_path(path))
}

fn strip_typescript_module_extension(path: &str) -> String {
    for suffix in [".tsx", ".ts", ".jsx", ".js"] {
        if let Some(stripped) = path.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    path.to_string()
}

fn contains_member_call_name(body_text: &str, object_name: &str, method_name: &str) -> bool {
    let needle = format!("{object_name}.{method_name}(");
    body_text.match_indices(&needle).any(|(idx, _)| {
        has_member_call_boundary(body_text, idx)
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
            && !inside_block_comment(body_text, idx)
    })
}

fn has_member_call_boundary(body_text: &str, idx: usize) -> bool {
    body_text[..idx]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_javascript_identifier_char(ch) && ch != '.')
}

fn line_prefix_looks_like_comment_or_string(body_text: &str, idx: usize) -> bool {
    let line_start = body_text[..idx].rfind('\n').map_or(0, |offset| offset + 1);
    let prefix = &body_text[line_start..idx];
    prefix.trim_start().starts_with("//") || has_unclosed_quote_or_template(prefix)
}

fn inside_block_comment(body_text: &str, idx: usize) -> bool {
    let prefix = &body_text[..idx];
    let comment_start = prefix.rfind("/*");
    let comment_end = prefix.rfind("*/");
    comment_start.is_some_and(|start| comment_end.is_none_or(|end| start > end))
}

fn has_unclosed_quote_or_template(prefix: &str) -> bool {
    let mut escaped = false;
    let mut in_single = false;
    let mut in_double = false;
    let mut in_template = false;
    for ch in prefix.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\'' && !in_double && !in_template {
            in_single = !in_single;
        } else if ch == '"' && !in_single && !in_template {
            in_double = !in_double;
        } else if ch == '`' && !in_single && !in_double {
            in_template = !in_template;
        }
    }
    in_single || in_double || in_template
}

fn is_javascript_identifier_char(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
}

fn assertion_oracle_text(assertion: &TypeScriptAssertion) -> String {
    if matches!(assertion.matcher.as_str(), "toThrow" | "toThrowError")
        && assertion.argument_count == 0
    {
        format!("expect(...).{}()", assertion.matcher)
    } else {
        format!("expect(...).{}(...)", assertion.matcher)
    }
}

/// Pick the highest-rank assertion from a test body. Used to summarise a
/// related test's strongest oracle for the classifier.
fn strongest_assertion(assertions: &[TypeScriptAssertion]) -> Option<&TypeScriptAssertion> {
    assertions
        .iter()
        .max_by_key(|assertion| assertion.oracle_strength.rank())
}

/// Collect the deduplicated set of module paths that any related test
/// file mocks via syntactic `vi.mock("path")` / `jest.mock("path")`.
///
/// Related tests are identified the same way `find_related_tests` does
/// (by name-call reference to the owner); each test's
/// `mocks_in_file` list is contributed once. The classifier uses the
/// resulting list to surface the `mocked_module` static-limit per
/// RIPR-SPEC-0026.
fn collect_related_mock_paths(
    owner: &TypeScriptOwner,
    all_tests: &[TypeScriptTest],
) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for test in all_tests
        .iter()
        .filter(|test| test_references_owner(test, owner))
    {
        for path in &test.mocks_in_file {
            if !paths.iter().any(|existing| existing == path) {
                paths.push(path.clone());
            }
        }
    }
    paths
}

/// Syntax-first probe-family classifier for a changed line of TypeScript
/// or JavaScript source.
///
/// Inspects the leading non-whitespace tokens of `line_text` and falls
/// back to substring shape checks for ternary / arrow-bodied expressions.
/// Matches the families documented in RIPR-SPEC-0027 and pinned by the
/// `typescript_probe_shapes_*` fixture row of #768.
///
/// The adapter operates without a type checker, so this classifier is
/// intentionally conservative: ambiguous shapes fall through to
/// `Predicate` / `Control` (the default established by the owner+test
/// sub-slice in #777) rather than guessing across families.
fn classify_probe_shape(line_text: &str) -> (ProbeFamily, DeltaKind) {
    let trimmed = line_text.trim_start();
    // Strip a leading `} ` (e.g., `} else if (...)`, `} else {`) so the
    // dedicated-keyword check still fires on close-brace-continuation
    // shapes that are common in JavaScript-style if/else ladders.
    let leading = trimmed.strip_prefix("} ").unwrap_or(trimmed).trim_start();

    if leading.starts_with("throw ")
        || leading.starts_with("throw(")
        || leading.starts_with("return Promise.reject(")
        || leading.starts_with("return Promise.reject ")
        || leading.starts_with("return await Promise.reject(")
        || leading.starts_with("return await Promise.reject ")
        || leading.starts_with("await Promise.reject(")
        || leading.starts_with("await Promise.reject ")
        || leading.starts_with("} catch ")
        || leading.starts_with("catch ")
    {
        return (ProbeFamily::ErrorPath, DeltaKind::Control);
    }
    if leading.starts_with("return ") || leading == "return;" || leading.starts_with("return;") {
        return (ProbeFamily::ReturnValue, DeltaKind::Value);
    }
    if leading.starts_with("if (")
        || leading.starts_with("if(")
        || leading.starts_with("else if (")
        || leading.starts_with("else if(")
        || leading.starts_with("while (")
        || leading.starts_with("while(")
        || leading.starts_with("for (")
        || leading.starts_with("for(")
        || leading.starts_with("switch (")
        || leading.starts_with("switch(")
        || leading.starts_with("case ")
        || leading.starts_with("default:")
    {
        return (ProbeFamily::Predicate, DeltaKind::Control);
    }
    // Top-level ternary or short-circuit expression that is *not* embedded
    // in a `return` or assignment — treat as a predicate boundary.
    if (leading.contains("? ") && leading.contains(" : "))
        && !leading.starts_with("const ")
        && !leading.starts_with("let ")
        && !leading.starts_with("var ")
    {
        return (ProbeFamily::Predicate, DeltaKind::Control);
    }
    // Field / property assignments: `this.x = ...`, `obj.x = ...`, or
    // top-level binding declarations inside a constructor / setter body.
    // Detected only when the line has the form `<ident chain> = <expr>`
    // without a leading function-call shape; this keeps statement-level
    // call expressions in the SideEffect bucket below.
    if let Some(eq_idx) = leading.find(" = ")
        && !leading.starts_with("if ")
        && !leading.starts_with("else ")
        && !leading.starts_with("return")
        && !leading.starts_with("throw")
    {
        let lhs = &leading[..eq_idx];
        let looks_like_assignment = lhs
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '[' || c == ']');
        let looks_like_declaration =
            lhs.starts_with("const ") || lhs.starts_with("let ") || lhs.starts_with("var ");
        if looks_like_assignment && !looks_like_declaration {
            return (ProbeFamily::FieldConstruction, DeltaKind::Value);
        }
    }
    // Bare call-expression statement (e.g., `tracker.record(event);`,
    // `await logger.flush();`). Detected by trailing `);` after stripping
    // optional `await ` / `void ` / trailing comments.
    let call_candidate = leading
        .strip_prefix("await ")
        .unwrap_or(leading)
        .strip_prefix("void ")
        .unwrap_or_else(|| leading.strip_prefix("await ").unwrap_or(leading))
        .trim_end();
    let call_candidate = call_candidate
        .strip_suffix(';')
        .unwrap_or(call_candidate)
        .trim_end();
    if call_candidate.ends_with(')')
        && call_candidate.contains('(')
        && !call_candidate.starts_with("if")
        && !call_candidate.starts_with("while")
        && !call_candidate.starts_with("for")
        && !call_candidate.starts_with("switch")
        && !call_candidate.starts_with("return")
        && !call_candidate.starts_with("throw")
        && !call_candidate.starts_with("const ")
        && !call_candidate.starts_with("let ")
        && !call_candidate.starts_with("var ")
    {
        return (ProbeFamily::SideEffect, DeltaKind::Effect);
    }
    // Fall through: keep the pre-#768 default. The adapter does not yet
    // recognise this shape, so flagging it as a generic predicate-control
    // change matches the owner+test sub-slice baseline rather than
    // committing to a more specific family the adapter cannot confirm.
    (ProbeFamily::Predicate, DeltaKind::Control)
}

fn classify_change(
    file: &Path,
    line: usize,
    line_text: &str,
    owners: &[TypeScriptOwner],
    all_tests: &[TypeScriptTest],
) -> Option<Finding> {
    let changed_file = normalized_path(file);
    let owner = owners
        .iter()
        .filter(|owner| normalized_path(&owner.file) == changed_file)
        .find(|owner| line >= owner.start_line && line <= owner.end_line)?;
    let related = find_related_tests(owner, all_tests);
    let mock_paths = collect_related_mock_paths(owner, all_tests);

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
            vec![no_static_path_missing(owner)],
        )
    } else if strongest_strength >= OracleStrength::Strong.rank() {
        (
            ExposureClass::Exposed,
            StageState::Yes,
            StageState::Yes,
            StageState::Yes,
            vec![format!(
                "Related test reaches `{}` with a `{}` oracle. Static evidence suggests the changed behavior is observed under an exact-value or exact-error-variant discriminator.",
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
                "Related test reaches `{}` but the strongest extracted oracle is `{}`; upgrade by adding an exact-value (`toBe` / `toEqual` / `toStrictEqual`) assertion. TypeScript `toThrow` forms remain broad error evidence until payload inspection lands.",
                owner.name,
                strongest_kind.as_str()
            )],
        )
    };
    if !mock_paths.is_empty() {
        let preview: String = mock_paths
            .iter()
            .map(|path| format!("`{path}`"))
            .collect::<Vec<_>>()
            .join(", ");
        missing.push(format!(
            "Static limit `mocked_module`: related test file mocks {preview} via `vi.mock(...)` / `jest.mock(...)`. The TypeScript preview adapter does not resolve mocked module semantics, so the substitution under test is opaque to static evidence."
        ));
    }

    let id_path: String = file
        .display()
        .to_string()
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let (family, delta) = classify_probe_shape(line_text);
    let probe = Probe {
        id: ProbeId(format!("probe:{id_path}:{line}:typescript_preview")),
        location: SourceLocation::new(file.to_string_lossy().as_ref(), line, 1),
        owner: Some(owner.symbol_id()),
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
        "{} related test(s) found for owner `{}`",
        related_count, owner.name
    );
    let reach = StageEvidence::new(reach_state.clone(), Confidence::Low, &reach_summary);
    let infect = StageEvidence::new(
        StageState::Unknown,
        Confidence::Low,
        "TypeScript preview adapter does not yet model infection.",
    );
    let propagate = StageEvidence::new(
        StageState::Unknown,
        Confidence::Low,
        "TypeScript preview adapter does not yet model propagation.",
    );
    let observe_summary = format!(
        "Strongest extracted oracle kind: `{}` (rank {})",
        strongest_kind.as_str(),
        strongest_strength
    );
    let observe = StageEvidence::new(observe_state, Confidence::Low, &observe_summary);
    let discriminate_summary = if strongest_strength >= OracleStrength::Strong.rank() {
        format!(
            "Related test uses a `{}` oracle; static evidence suggests the changed behavior is discriminated.",
            strongest_kind.as_str()
        )
    } else {
        "TypeScript preview adapter found no strong discriminator; use `toBe` / `toEqual` / `toStrictEqual` to escalate. TypeScript `toThrow` forms remain broad error evidence until payload inspection lands.".to_string()
    };
    let discriminate =
        StageEvidence::new(discriminate_state, Confidence::Low, &discriminate_summary);

    let recommended = match &class {
        ExposureClass::Exposed => {
            "TypeScript preview: changed behavior is observed under a strong oracle; verify the assertion targets the changed boundary value.".to_string()
        }
        ExposureClass::NoStaticPath => {
            no_static_path_recommendation(owner)
        }
        _ => {
            "TypeScript preview: add a test that exercises the changed behavior with an exact-value assertion (`toBe` / `toEqual` / `toStrictEqual`).".to_string()
        }
    };
    let confidence_value = if matches!(class, ExposureClass::Exposed) {
        0.6
    } else {
        0.4
    };

    let mut evidence = vec![format!("owner: {}", owner.name)];
    for path in &mock_paths {
        evidence.push(format!("static_limit mocked_module: `{path}`"));
    }
    Some(Finding {
        id: probe.id.0.clone(),
        canonical_gap: None,
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
        language: Some(output_language_for(file)),
        language_status: Some(LanguageStatus::Preview),
        owner_kind: Some(owner.owner_kind),
        static_limit_kind: (!mock_paths.is_empty()).then_some(StaticLimitKind::MockedModule),
    })
}

fn no_static_path_missing(owner: &TypeScriptOwner) -> String {
    match owner.owner_kind {
        OwnerKind::Method | OwnerKind::ClassMethod => format!(
            "No trusted TypeScript method relation for `{}`. Object method calls stay ambiguous in preview until method related-test matching lands.",
            owner.name
        ),
        OwnerKind::ModuleFunction => format!(
            "No trusted TypeScript module-initializer relation for `{}`. Identifier-reference observers stay missing context until module-value related-test matching lands.",
            owner.name
        ),
        _ => format!(
            "No test references `{}(` — add a test that calls the changed owner.",
            owner.name
        ),
    }
}

fn no_static_path_recommendation(owner: &TypeScriptOwner) -> String {
    match owner.owner_kind {
        OwnerKind::Method | OwnerKind::ClassMethod => {
            "TypeScript preview: method owner relation is missing context; add an exact method observer only after the adapter can safely relate the receiver shape.".to_string()
        }
        OwnerKind::ModuleFunction => {
            "TypeScript preview: module initializer relation is missing context; add an exact value observer and keep the finding advisory until identifier-reference matching lands.".to_string()
        }
        _ => {
            "TypeScript preview: no test references the changed owner; add a test that calls the owner and asserts the changed behavior with `toBe` / `toEqual`.".to_string()
        }
    }
}

fn output_language_for(path: &Path) -> DomainLanguageId {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("js" | "jsx") => DomainLanguageId::JavaScript,
        _ => DomainLanguageId::TypeScript,
    }
}

fn parse_limit_for_file<'a>(
    file: &Path,
    limits: &'a [TypeScriptParseLimit],
) -> Option<&'a TypeScriptParseLimit> {
    let changed_file = normalized_path(file);
    limits
        .iter()
        .find(|limit| normalized_path(&limit.file) == changed_file)
}

fn unsupported_syntax_finding(
    file: &Path,
    line: usize,
    line_text: &str,
    limit: &TypeScriptParseLimit,
) -> Finding {
    let id_path: String = file
        .display()
        .to_string()
        .chars()
        .map(|c| if c == '/' || c == '\\' { '_' } else { c })
        .collect();
    let probe = Probe {
        id: ProbeId(format!(
            "probe:{id_path}:{line}:typescript_preview_unsupported_syntax"
        )),
        location: SourceLocation::new(file.to_string_lossy().as_ref(), line, 1),
        owner: None,
        family: ProbeFamily::StaticUnknown,
        delta: DeltaKind::Unknown,
        before: None,
        after: Some(line_text.to_string()),
        expression: line_text.to_string(),
        expected_sinks: Vec::new(),
        required_oracles: Vec::new(),
    };
    let summary = format!(
        "TypeScript preview parser could not build syntax facts for `{}`: {}",
        normalized_path(file),
        limit.reason
    );
    let stage = StageEvidence::new(StageState::Unknown, Confidence::Low, &summary);
    let missing = format!(
        "Static limit `unsupported_syntax`: malformed TypeScript/JavaScript prevented syntax-first owner, test, and probe extraction for `{}`.",
        normalized_path(file)
    );
    let recommended = "TypeScript preview: fix or isolate the unsupported syntax before relying on repair guidance; no actionable repair packet is emitted.".to_string();

    Finding {
        id: probe.id.0.clone(),
        canonical_gap: None,
        probe,
        class: ExposureClass::StaticUnknown,
        ripr: RiprEvidence {
            reach: stage.clone(),
            infect: stage.clone(),
            propagate: stage.clone(),
            reveal: RevealEvidence {
                observe: stage.clone(),
                discriminate: stage,
            },
        },
        confidence: 0.2,
        evidence: vec![format!("static_limit unsupported_syntax: {}", limit.reason)],
        missing: vec![missing],
        flow_sinks: Vec::new(),
        activation: Default::default(),
        stop_reasons: vec![StopReason::StaticProbeUnknown],
        related_tests: Vec::new(),
        recommended_next_step: Some(recommended),
        language: Some(output_language_for(file)),
        language_status: Some(LanguageStatus::Preview),
        owner_kind: None,
        static_limit_kind: Some(StaticLimitKind::UnsupportedSyntax),
    }
}

fn collect_workspace_typescript_files(root: &Path) -> Vec<PathBuf> {
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
            let adapter = TypeScriptAdapter;
            if adapter.accepts_path(&path) {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                out.push(relative);
            }
        }
    }
}

impl LanguageAdapter for TypeScriptAdapter {
    fn accepts_path(&self, path: &Path) -> bool {
        matches!(route(path), Some(LanguageId::TypeScript))
    }

    fn analyze_diff(
        &self,
        options: &AnalysisOptions,
        _oracle_policy: &OraclePolicy,
        changed_files: &[ChangedFile],
    ) -> Result<LanguageDiffResult, String> {
        // Phase 1: discover and index every accepted file in the workspace
        // so we can find related tests for any owner regardless of whether
        // the test file itself changed in this diff.
        let workspace_files = collect_workspace_typescript_files(&options.root);
        let mut all_owners: Vec<TypeScriptOwner> = Vec::new();
        let mut all_tests: Vec<TypeScriptTest> = Vec::new();
        let mut parse_limits: Vec<TypeScriptParseLimit> = Vec::new();
        for relative in &workspace_files {
            let absolute = options.root.join(relative);
            let Ok(source) = std::fs::read_to_string(&absolute) else {
                continue;
            };
            if let Some(reason) = parse_error_reason(relative, &source) {
                if !is_test_file(relative) {
                    parse_limits.push(TypeScriptParseLimit {
                        file: relative.clone(),
                        reason,
                    });
                }
                continue;
            }
            if is_test_file(relative) {
                all_tests.extend(extract_tests(relative, &source));
            } else {
                all_owners.extend(extract_owners(relative, &source));
            }
        }

        // Phase 2: for each accepted changed file, classify each changed
        // line that falls inside an owner.
        let mut findings: Vec<Finding> = Vec::new();
        let mut changed_count: usize = 0;
        for changed in changed_files {
            if !self.accepts_path(&changed.path) {
                continue;
            }
            changed_count += 1;
            // Skip test-file changes for finding generation; classifier
            // operates on production owners. Test file edits are still
            // counted in the file tally.
            if is_test_file(&changed.path) {
                continue;
            }
            if let Some(limit) = parse_limit_for_file(&changed.path, &parse_limits) {
                if let Some(added) = changed.added_lines.first() {
                    findings.push(unsupported_syntax_finding(
                        &changed.path,
                        added.line,
                        &added.text,
                        limit,
                    ));
                }
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
    fn accepts_ts_jsx_paths() {
        let adapter = TypeScriptAdapter;
        assert!(adapter.accepts_path(Path::new("src/index.ts")));
        assert!(adapter.accepts_path(Path::new("src/component.tsx")));
        assert!(adapter.accepts_path(Path::new("src/index.js")));
        assert!(adapter.accepts_path(Path::new("src/component.jsx")));
        assert!(!adapter.accepts_path(Path::new("src/lib.rs")));
        assert!(!adapter.accepts_path(Path::new("scripts/run.py")));
        assert!(!adapter.accepts_path(Path::new("README.md")));
    }

    #[test]
    fn extract_owners_returns_empty_when_source_does_not_parse() {
        let owners = extract_owners(
            Path::new("src/index.ts"),
            "this is not :: valid +++ typescript",
        );
        assert!(owners.is_empty());
    }

    #[test]
    fn parse_error_reason_reports_parser_errors() {
        let reason = parse_error_reason(
            Path::new("src/index.ts"),
            "this is not :: valid +++ typescript",
        );
        assert!(reason.is_some());
        let reason = reason.unwrap_or_default();
        assert!(reason.contains("parser error"));
    }

    #[test]
    fn unsupported_syntax_finding_is_preview_static_unknown() {
        let limit = TypeScriptParseLimit {
            file: PathBuf::from("src/index.ts"),
            reason: "1 parser error(s)".to_string(),
        };
        let finding =
            unsupported_syntax_finding(Path::new("src/index.ts"), 3, "  const value = ;", &limit);

        assert!(matches!(finding.class, ExposureClass::StaticUnknown));
        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::UnsupportedSyntax)
        );
        assert_eq!(finding.language, Some(DomainLanguageId::TypeScript));
        assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
        assert_eq!(finding.stop_reasons, vec![StopReason::StaticProbeUnknown]);
    }

    #[test]
    fn is_test_file_matches_test_and_spec_suffixes() {
        assert!(is_test_file(Path::new("tests/lib.test.ts")));
        assert!(is_test_file(Path::new("src/Header.spec.tsx")));
        assert!(is_test_file(Path::new("legacy.test.js")));
        assert!(!is_test_file(Path::new("src/lib.ts")));
        assert!(!is_test_file(Path::new("README.md")));
    }

    #[test]
    fn line_for_offset_counts_newlines() {
        let source = "line1\nline2\nline3\n";
        assert_eq!(line_for_offset(source, 0), 1);
        assert_eq!(line_for_offset(source, 5), 1);
        assert_eq!(line_for_offset(source, 6), 2);
        assert_eq!(line_for_offset(source, 12), 3);
    }

    #[test]
    fn normalized_path_strips_dot_prefix_and_normalizes_separators() {
        assert_eq!(normalized_path(Path::new(r".\src\b.ts")), "src/b.ts");
    }

    #[test]
    fn extract_owners_recognizes_function_declaration() {
        let owners = extract_owners(
            Path::new("src/lib.ts"),
            "function applyDiscount(amount: number): number {\n    return amount;\n}\n",
        );
        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].name, "applyDiscount");
        assert_eq!(owners[0].start_line, 1);
        assert_eq!(owners[0].owner_kind, OwnerKind::Function);
    }

    #[test]
    fn extract_owners_recognizes_exported_function() {
        let owners = extract_owners(
            Path::new("src/lib.ts"),
            "export function publicHelper(): void {}\n",
        );
        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].name, "publicHelper");
        assert_eq!(owners[0].owner_kind, OwnerKind::Function);
    }

    #[test]
    fn extract_owners_recognizes_arrow_const_and_module_initializer() {
        let owners = extract_owners(
            Path::new("src/lib.ts"),
            r#"const formatPrice = (amount: number) => {
    return amount.toFixed(2);
};
const defaultRate = 0.08;
"#,
        );
        assert_eq!(owners.len(), 2);
        assert_eq!(owners[0].name, "formatPrice");
        assert_eq!(owners[0].owner_kind, OwnerKind::ArrowFunction);
        assert_eq!(owners[0].start_line, 1);
        assert_eq!(owners[0].end_line, 3);
        assert_eq!(owners[1].name, "defaultRate");
        assert_eq!(owners[1].owner_kind, OwnerKind::ModuleFunction);
        assert_eq!(owners[1].start_line, 4);
    }

    #[test]
    fn extract_owners_recognizes_class_methods() {
        let owners = extract_owners(
            Path::new("src/cart.ts"),
            r#"class Cart {
    total() {
        return 1;
    }
    static build() {
        return new Cart();
    }
}
"#,
        );
        assert_eq!(owners.len(), 2);
        assert_eq!(owners[0].name, "total");
        assert_eq!(owners[0].owner_kind, OwnerKind::Method);
        assert_eq!(owners[0].start_line, 2);
        assert_eq!(owners[1].name, "build");
        assert_eq!(owners[1].owner_kind, OwnerKind::ClassMethod);
        assert_eq!(owners[1].start_line, 5);
    }

    #[test]
    fn extract_owners_recognizes_default_function_and_class_methods() {
        let function_owners = extract_owners(
            Path::new("src/defaults.ts"),
            r#"export default function calculate(value: number) {
    return value + 1;
}
"#,
        );
        let class_owners = extract_owners(
            Path::new("src/default-class.ts"),
            r#"
export default class Formatter {
    render() {
        return "ok";
    }
}
"#,
        );
        assert_eq!(function_owners.len(), 1);
        assert_eq!(function_owners[0].name, "calculate");
        assert_eq!(function_owners[0].owner_kind, OwnerKind::Function);
        assert_eq!(class_owners.len(), 1);
        assert_eq!(class_owners[0].name, "render");
        assert_eq!(class_owners[0].owner_kind, OwnerKind::Method);
    }

    #[test]
    fn extract_owners_recognizes_reactish_function_and_arrow_components() {
        let owners = extract_owners(
            Path::new("src/card.tsx"),
            r#"export function PriceTag() {
    return <span>price</span>;
}
const InlinePrice = () => (
    <span>price</span>
);
"#,
        );
        assert_eq!(owners.len(), 2);
        assert_eq!(owners[0].name, "PriceTag");
        assert_eq!(owners[0].owner_kind, OwnerKind::Component);
        assert_eq!(owners[1].name, "InlinePrice");
        assert_eq!(owners[1].owner_kind, OwnerKind::Component);
    }

    #[test]
    fn extract_owners_does_not_create_owner_from_comments_or_strings() {
        let owners = extract_owners(
            Path::new("src/docs.ts"),
            r#"// function fakeOwner() {}
"function stringOwner() {}";
"#,
        );
        assert!(owners.is_empty());
    }

    #[test]
    fn extract_tests_recognizes_test_and_it_blocks() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("alpha", () => { expect(applyDiscount(50, 100)).toBe(50); });
it("beta", () => { expect(otherHelper()).toBe(true); });
"#,
        );
        assert_eq!(tests.len(), 2);
        assert_eq!(tests[0].name, "alpha");
        assert_eq!(tests[1].name, "beta");
        assert!(tests[0].body_text.contains("applyDiscount(50, 100)"));
    }

    #[test]
    fn find_related_tests_matches_by_call_name() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let tests = vec![
            TypeScriptTest {
                name: "alpha".to_string(),
                file: PathBuf::from("tests/lib.test.ts"),
                line: 1,
                body_text: r#"test("alpha", () => { expect(applyDiscount(50, 100)).toBe(50); });"#
                    .to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
                imports_in_file: Vec::new(),
            },
            TypeScriptTest {
                name: "unrelated".to_string(),
                file: PathBuf::from("tests/other.test.ts"),
                line: 1,
                body_text: r#"test("unrelated", () => { expect(otherHelper()).toBe(true); });"#
                    .to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
                imports_in_file: Vec::new(),
            },
        ];
        let related = find_related_tests(&owner, &tests);
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "alpha");
    }

    #[test]
    fn find_related_tests_ignores_object_method_calls_for_function_owners() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let tests = vec![TypeScriptTest {
            name: "method call on another object".to_string(),
            file: PathBuf::from("tests/cart.test.ts"),
            line: 1,
            body_text: "expect(order.applyDiscount(50)).toBe(40);".to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        }];

        let related = find_related_tests(&owner, &tests);

        assert!(related.is_empty());
    }

    #[test]
    fn find_related_tests_matches_named_import_alias_calls() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/pricing.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let tests = extract_tests(
            Path::new("tests/pricing.test.ts"),
            r#"import { applyDiscount as subject } from "../src/pricing";

test("alias import observes threshold", () => {
    expect(subject(100, 100)).toBe(90);
});
"#,
        );

        let related = find_related_tests(&owner, &tests);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "alias import observes threshold");
    }

    #[test]
    fn find_related_tests_matches_namespace_import_member_calls() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/pricing.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let tests = extract_tests(
            Path::new("tests/pricing.test.ts"),
            r#"import * as pricing from "../src/pricing";

test("namespace import observes threshold", () => {
    expect(pricing.applyDiscount(100, 100)).toBe(90);
});
"#,
        );

        let related = find_related_tests(&owner, &tests);

        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "namespace import observes threshold");
    }

    #[test]
    fn find_related_tests_ignores_unrelated_and_type_only_import_aliases() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/pricing.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let tests = extract_tests(
            Path::new("tests/pricing.test.ts"),
            r#"import { applyDiscount as otherSubject } from "../src/other-pricing";
import type { applyDiscount as typeOnlySubject } from "../src/pricing";
import { applyDiscount } from "../src/other-pricing";

test("wrong import source", () => {
    expect(otherSubject(100, 100)).toBe(90);
});

test("wrong direct import source", () => {
    expect(applyDiscount(100, 100)).toBe(90);
});

test("type only import", () => {
    expect(typeOnlySubject(100, 100)).toBe(90);
});
"#,
        );

        let related = find_related_tests(&owner, &tests);

        assert!(related.is_empty());
    }

    #[test]
    fn find_related_tests_ignores_call_shaped_string_mentions() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let tests = vec![TypeScriptTest {
            name: "string mention".to_string(),
            file: PathBuf::from("tests/docs.test.ts"),
            line: 1,
            body_text: r#"expect("applyDiscount(").toContain("applyDiscount(");"#.to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        }];

        let related = find_related_tests(&owner, &tests);

        assert!(related.is_empty());
    }

    #[test]
    fn find_related_tests_ignores_call_shaped_comment_mentions() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let tests = vec![
            TypeScriptTest {
                name: "line comment mention".to_string(),
                file: PathBuf::from("tests/docs.test.ts"),
                line: 1,
                body_text: "// applyDiscount(\nexpect(total).toBe(40);".to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
                imports_in_file: Vec::new(),
            },
            TypeScriptTest {
                name: "block comment mention".to_string(),
                file: PathBuf::from("tests/docs.test.ts"),
                line: 4,
                body_text: "/* applyDiscount(\n */\nexpect(total).toBe(40);".to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
                imports_in_file: Vec::new(),
            },
        ];

        let related = find_related_tests(&owner, &tests);

        assert!(related.is_empty());
    }

    #[test]
    fn classify_change_returns_weakly_exposed_when_related_test_exists() -> Result<(), String> {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let test = TypeScriptTest {
            name: "alpha".to_string(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: "applyDiscount(50, 100)".to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        };
        let finding = classify_change(
            Path::new("src/lib.ts"),
            2,
            "    if (amount >= threshold) {",
            &[owner],
            &[test],
        )
        .ok_or_else(|| "expected a finding when an owner contains the changed line".to_string())?;
        assert!(matches!(finding.class, ExposureClass::WeaklyExposed));
        assert_eq!(finding.language, Some(DomainLanguageId::TypeScript));
        assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
        assert_eq!(finding.related_tests.len(), 1);
        Ok(())
    }

    #[test]
    fn classify_change_labels_javascript_sources_separately() -> Result<(), String> {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.js"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let test = TypeScriptTest {
            name: "alpha".to_string(),
            file: PathBuf::from("tests/lib.test.js"),
            line: 1,
            body_text: "applyDiscount(50, 100)".to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        };

        let finding = classify_change(
            Path::new("src/lib.js"),
            2,
            "    if (amount >= threshold) {",
            &[owner],
            &[test],
        )
        .ok_or_else(|| "expected a JavaScript preview finding".to_string())?;

        assert_eq!(finding.language, Some(DomainLanguageId::JavaScript));
        assert_eq!(finding.language_status, Some(LanguageStatus::Preview));
        Ok(())
    }

    #[test]
    fn classify_change_matches_owner_file_before_line_range() -> Result<(), String> {
        let owners = vec![
            TypeScriptOwner {
                name: "alphaScore".to_string(),
                file: PathBuf::from("src/a.ts"),
                start_line: 1,
                end_line: 5,
                owner_kind: OwnerKind::Function,
            },
            TypeScriptOwner {
                name: "betaScore".to_string(),
                file: PathBuf::from("src/b.ts"),
                start_line: 1,
                end_line: 5,
                owner_kind: OwnerKind::Function,
            },
        ];
        let tests = vec![
            TypeScriptTest {
                name: "alpha keeps its threshold".to_string(),
                file: PathBuf::from("tests/a.test.ts"),
                line: 1,
                body_text: "expect(alphaScore(12)).toBe(13);".to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
                imports_in_file: Vec::new(),
            },
            TypeScriptTest {
                name: "beta keeps its threshold".to_string(),
                file: PathBuf::from("tests/b.test.ts"),
                line: 1,
                body_text: "expect(betaScore(12)).toBe(13);".to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
                imports_in_file: Vec::new(),
            },
        ];

        let finding = classify_change(
            Path::new("src/b.ts"),
            2,
            "    if (value >= 10) {",
            &owners,
            &tests,
        )
        .ok_or_else(|| "expected the changed file's owner to be selected".to_string())?;

        assert_eq!(finding.evidence, vec!["owner: betaScore"]);
        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(finding.related_tests[0].name, "beta keeps its threshold");
        assert_eq!(
            finding.related_tests[0].file,
            PathBuf::from("tests/b.test.ts")
        );
        assert!(finding.missing.iter().all(|line| !line.contains("alpha")));
        Ok(())
    }

    #[test]
    fn extract_tests_collects_expect_to_be_as_strong_oracle() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("alpha", () => {
    expect(applyDiscount(50, 100)).toBe(50);
    expect(applyDiscount(10000, 100)).toEqual(9990);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 2);
        assert_eq!(tests[0].assertions[0].matcher, "toBe");
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(
            tests[0].assertions[0].oracle_strength,
            OracleStrength::Strong
        );
        assert_eq!(tests[0].assertions[1].matcher, "toEqual");
    }

    #[test]
    fn extract_tests_recurses_nested_describe_blocks() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"describe("pricing", () => {
    describe("discounts", () => {
        it("pins threshold", () => {
            expect(applyDiscount(100, 100)).toStrictEqual(90);
        });
    });
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].name, "pricing discounts pins threshold");
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toStrictEqual");
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
    }

    #[test]
    fn extract_tests_recognizes_test_each_table_calls() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test.each([
    [100, 100, 90],
    [150, 100, 140],
])("discounts %#", (amount, threshold, expected) => {
    expect(applyDiscount(amount, threshold)).toBe(expected);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].name, "discounts %#");
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toBe");
        assert!(tests[0].body_text.contains("applyDiscount("));
    }

    #[test]
    fn extract_tests_recognizes_it_each_table_calls() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"it.each([
    ["ready"],
])("notifies %s", (status) => {
    const sink = { record: vi.fn() };
    notifyStatus(status, sink);
    expect(sink.record).toHaveBeenCalledWith(status);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].name, "notifies %s");
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toHaveBeenCalledWith");
        assert_eq!(
            tests[0].assertions[0].oracle_kind,
            OracleKind::MockExpectation
        );
    }

    #[test]
    fn extract_tests_recognizes_resolves_async_chain() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("async", async () => {
    await expect(loader()).resolves.toBe(42);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toBe");
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
    }

    #[test]
    fn extract_tests_recognizes_return_await_resolves_async_chain() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("async return", async () => {
    return await expect(loader()).resolves.toBe(42);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toBe");
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::ExactValue);
    }

    #[test]
    fn extract_tests_collects_assertions_nested_in_control_flow() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("nested", () => {
    if (enabled) {
        expect(applyDiscount(50, 100)).toBe(50);
    } else {
        expect(applyDiscount(1, 100)).toEqual(1);
    }
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 2);
        assert_eq!(tests[0].assertions[0].matcher, "toBe");
        assert_eq!(tests[0].assertions[1].matcher, "toEqual");
    }

    #[test]
    fn extract_tests_collects_assertions_nested_in_loop_switch_and_label_bodies() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("nested statements", () => {
    while (enabled) {
        expect(loopValue).toBe(1);
    }
    do {
        expect(done).toBeTruthy();
    } while (retry);
    for (let index = 0; index < items.length; index++) {
        expect(items[index]).toBeDefined();
    }
    for (const key in record) {
        expect(record[key]).toEqual("value");
    }
    for (const item of items) {
        expect(item).toBeDefined();
    }
    retry: {
        expect(labelled).toBe(false);
    }
    switch (kind) {
        case "a":
            expect(kind).toBe("a");
            break;
        default:
            expect(kind).toEqual("fallback");
    }
});
"#,
        );
        assert_eq!(tests.len(), 1);
        let matchers: Vec<&str> = tests[0]
            .assertions
            .iter()
            .map(|assertion| assertion.matcher.as_str())
            .collect();
        assert_eq!(
            matchers,
            vec![
                "toBe",
                "toBeTruthy",
                "toBeDefined",
                "toEqual",
                "toBeDefined",
                "toBe",
                "toBe",
                "toEqual"
            ]
        );
    }

    #[test]
    fn extract_tests_collects_assertions_nested_in_try_catch_finally() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("try-catch", () => {
    try {
        expect(parseUser("Ada")).toEqual({ name: "Ada" });
    } catch (err) {
        expect(err).toBeDefined();
    } finally {
        expect(cleanup).toHaveBeenCalled();
    }
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 3);
        assert_eq!(tests[0].assertions[0].matcher, "toEqual");
        assert_eq!(tests[0].assertions[1].matcher, "toBeDefined");
        assert_eq!(tests[0].assertions[2].matcher, "toHaveBeenCalled");
    }

    #[test]
    fn extract_tests_unknown_matcher_maps_to_unknown_oracle() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("alpha", () => {
    expect(applyDiscount(50, 100)).customDomainMatcher(50);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::Unknown);
        assert_eq!(
            tests[0].assertions[0].oracle_strength,
            OracleStrength::Unknown
        );
    }

    #[test]
    fn extract_tests_maps_bare_tothrow_to_broad_error_oracle() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("throws", () => {
    expect(() => parseUser("")).toThrow();
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toThrow");
        assert_eq!(tests[0].assertions[0].argument_count, 0);
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::BroadError);
        assert_eq!(tests[0].assertions[0].oracle_strength, OracleStrength::Weak);
    }

    #[test]
    fn extract_tests_keeps_payload_tothrow_broad_until_payload_is_inspected() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("throws", () => {
    expect(() => parseUser("")).toThrow("empty user");
});
"#,
        );
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].assertions.len(), 1);
        assert_eq!(tests[0].assertions[0].matcher, "toThrow");
        assert_eq!(tests[0].assertions[0].argument_count, 1);
        assert_eq!(tests[0].assertions[0].oracle_kind, OracleKind::BroadError);
        assert_eq!(tests[0].assertions[0].oracle_strength, OracleStrength::Weak);
    }

    #[test]
    fn oracle_for_matcher_covers_canonical_jest_vitest_set() {
        assert_eq!(
            oracle_for_matcher("toBe"),
            (OracleKind::ExactValue, OracleStrength::Strong)
        );
        assert_eq!(
            oracle_for_matcher("toEqual"),
            (OracleKind::ExactValue, OracleStrength::Strong)
        );
        assert_eq!(
            oracle_for_matcher("toThrow"),
            (OracleKind::BroadError, OracleStrength::Weak)
        );
        assert_eq!(
            oracle_for_matcher("toMatchSnapshot"),
            (OracleKind::Snapshot, OracleStrength::Medium)
        );
        assert_eq!(
            oracle_for_matcher("toHaveBeenCalledWith"),
            (OracleKind::MockExpectation, OracleStrength::Medium)
        );
        assert_eq!(
            oracle_for_matcher("toBeTruthy"),
            (OracleKind::SmokeOnly, OracleStrength::Smoke)
        );
        assert_eq!(
            oracle_for_matcher("toContain"),
            (OracleKind::RelationalCheck, OracleStrength::Weak)
        );
        assert_eq!(
            oracle_for_matcher("someUnknownMatcher"),
            (OracleKind::Unknown, OracleStrength::Unknown)
        );
    }

    #[test]
    fn classify_change_returns_exposed_when_related_test_has_strong_oracle() -> Result<(), String> {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let test = TypeScriptTest {
            name: "alpha".to_string(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: "applyDiscount(50, 100)".to_string(),
            assertions: vec![TypeScriptAssertion {
                matcher: "toBe".to_string(),
                argument_count: 1,
                line: 2,
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
            }],
            mocks_in_file: Vec::new(),
            imports_in_file: Vec::new(),
        };
        let finding = classify_change(
            Path::new("src/lib.ts"),
            2,
            "    if (amount >= threshold) {",
            &[owner],
            &[test],
        )
        .ok_or_else(|| "expected a finding for the changed line".to_string())?;
        assert!(matches!(finding.class, ExposureClass::Exposed));
        assert_eq!(finding.related_tests.len(), 1);
        assert_eq!(finding.related_tests[0].oracle_kind, OracleKind::ExactValue);
        assert_eq!(
            finding.related_tests[0].oracle_strength,
            OracleStrength::Strong
        );
        Ok(())
    }

    #[test]
    fn classify_change_returns_no_static_path_when_no_related_test() -> Result<(), String> {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let finding = classify_change(
            Path::new("src/lib.ts"),
            2,
            "    if (amount >= threshold) {",
            &[owner],
            &[],
        )
        .ok_or_else(|| "expected a finding when an owner contains the changed line".to_string())?;
        assert!(matches!(finding.class, ExposureClass::NoStaticPath));
        assert!(finding.related_tests.is_empty());
        Ok(())
    }

    #[test]
    fn classify_change_returns_none_when_line_is_outside_any_owner() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 10,
            end_line: 20,
            owner_kind: OwnerKind::Function,
        };
        let finding = classify_change(
            Path::new("src/lib.ts"),
            5,
            "// top-level comment",
            &[owner],
            &[],
        );
        assert!(finding.is_none());
    }

    #[test]
    fn analyze_diff_returns_zero_findings_and_counts_accepted_files() -> Result<(), String> {
        let adapter = TypeScriptAdapter;
        let options = AnalysisOptions {
            root: PathBuf::from("/nonexistent_workspace"),
            base: None,
            diff_file: None,
            mode: crate::analysis::AnalysisMode::Draft,
            include_unchanged_tests: false,
        };
        let policy = OraclePolicy::default();
        let changed_files = vec![
            changed("src/index.ts"),
            changed("src/lib.rs"),
            changed("docs/README.md"),
            changed("src/Header.tsx"),
        ];
        let result = adapter.analyze_diff(&options, &policy, &changed_files)?;
        // No workspace files on disk -> no findings; counted-file tally
        // still reflects accepted changed paths.
        assert!(result.findings.is_empty());
        assert_eq!(result.changed_files, 2);
        Ok(())
    }

    #[test]
    fn analyze_repo_returns_empty_scaffold() -> Result<(), String> {
        let adapter = TypeScriptAdapter;
        let options = AnalysisOptions {
            root: PathBuf::from("/nonexistent_workspace"),
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

    #[test]
    fn classify_probe_shape_recognises_if_predicate() {
        let (family, delta) = classify_probe_shape("    if (amount >= threshold) {");
        assert_eq!(family, ProbeFamily::Predicate);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognises_else_if_predicate() {
        let (family, delta) = classify_probe_shape("    } else if (amount === 0) {");
        assert_eq!(family, ProbeFamily::Predicate);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognises_return_value() {
        let (family, delta) = classify_probe_shape("    return amount - 10;");
        assert_eq!(family, ProbeFamily::ReturnValue);
        assert_eq!(delta, DeltaKind::Value);
    }

    #[test]
    fn classify_probe_shape_recognises_bare_return() {
        let (family, delta) = classify_probe_shape("    return;");
        assert_eq!(family, ProbeFamily::ReturnValue);
        assert_eq!(delta, DeltaKind::Value);
    }

    #[test]
    fn classify_probe_shape_recognises_throw_error_path() {
        let (family, delta) = classify_probe_shape("    throw new Error('out of range');");
        assert_eq!(family, ProbeFamily::ErrorPath);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognises_promise_reject_error_path() {
        let (family, delta) = classify_probe_shape("    return Promise.reject(new Error('boom'));");
        assert_eq!(family, ProbeFamily::ErrorPath);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognises_return_await_promise_reject_error_path() {
        let (family, delta) =
            classify_probe_shape("    return await Promise.reject(new Error('boom'));");
        assert_eq!(family, ProbeFamily::ErrorPath);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognises_bare_await_promise_reject_error_path() {
        let (family, delta) = classify_probe_shape("    await Promise.reject(new Error('boom'));");
        assert_eq!(family, ProbeFamily::ErrorPath);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_recognises_field_construction() {
        let (family, delta) = classify_probe_shape("    this.count = next;");
        assert_eq!(family, ProbeFamily::FieldConstruction);
        assert_eq!(delta, DeltaKind::Value);
    }

    #[test]
    fn classify_probe_shape_recognises_side_effect_call() {
        let (family, delta) = classify_probe_shape("    logger.record(event);");
        assert_eq!(family, ProbeFamily::SideEffect);
        assert_eq!(delta, DeltaKind::Effect);
    }

    #[test]
    fn classify_probe_shape_recognises_await_side_effect_call() {
        let (family, delta) = classify_probe_shape("    await logger.flush();");
        assert_eq!(family, ProbeFamily::SideEffect);
        assert_eq!(delta, DeltaKind::Effect);
    }

    #[test]
    fn classify_probe_shape_recognises_ternary_as_predicate() {
        let (family, delta) =
            classify_probe_shape("    amount >= threshold ? amount - 10 : amount;");
        assert_eq!(family, ProbeFamily::Predicate);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn classify_probe_shape_falls_through_to_predicate_default_for_const_decl() {
        // `const` declarations do not match a specific family in the
        // preview adapter; conservative fall-through keeps the historical
        // owner+test sub-slice default (#777) rather than guessing.
        let (family, delta) =
            classify_probe_shape("    const total = applyDiscount(amount, threshold);");
        assert_eq!(family, ProbeFamily::Predicate);
        assert_eq!(delta, DeltaKind::Control);
    }

    #[test]
    fn extract_tests_collects_vi_mock_paths_in_file() {
        let source = r#"
import { vi } from "vitest";
vi.mock("./api");
vi.mock("./logger");
test("alpha", () => {
    expect(applyDiscount(50, 100)).toBe(50);
});
"#;
        let tests = extract_tests(Path::new("tests/lib.test.ts"), source);
        assert_eq!(tests.len(), 1);
        assert_eq!(
            tests[0].mocks_in_file,
            vec!["./api".to_string(), "./logger".to_string()]
        );
    }

    #[test]
    fn extract_tests_collects_jest_mock_paths_in_file() {
        let source = r#"
jest.mock("./repository");
test("alpha", () => {
    expect(applyDiscount(50, 100)).toBe(50);
});
"#;
        let tests = extract_tests(Path::new("tests/lib.test.ts"), source);
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].mocks_in_file, vec!["./repository".to_string()]);
    }

    #[test]
    fn extract_tests_returns_empty_mock_list_when_no_mock_call() {
        let source = r#"
test("alpha", () => {
    expect(applyDiscount(50, 100)).toBe(50);
});
"#;
        let tests = extract_tests(Path::new("tests/lib.test.ts"), source);
        assert_eq!(tests.len(), 1);
        assert!(tests[0].mocks_in_file.is_empty());
    }

    #[test]
    fn collect_related_mock_paths_dedups_across_tests_in_same_file() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let tests = vec![
            TypeScriptTest {
                name: "alpha".to_string(),
                file: PathBuf::from("tests/lib.test.ts"),
                line: 1,
                body_text: "applyDiscount(1, 2)".to_string(),
                assertions: Vec::new(),
                mocks_in_file: vec!["./api".to_string()],
                imports_in_file: Vec::new(),
            },
            TypeScriptTest {
                name: "beta".to_string(),
                file: PathBuf::from("tests/lib.test.ts"),
                line: 2,
                body_text: "applyDiscount(3, 4)".to_string(),
                assertions: Vec::new(),
                mocks_in_file: vec!["./api".to_string()],
                imports_in_file: Vec::new(),
            },
        ];
        let paths = collect_related_mock_paths(&owner, &tests);
        assert_eq!(paths, vec!["./api".to_string()]);
    }

    #[test]
    fn collect_related_mock_paths_ignores_unrelated_tests() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let tests = vec![TypeScriptTest {
            name: "unrelated".to_string(),
            file: PathBuf::from("tests/other.test.ts"),
            line: 1,
            body_text: "otherHelper()".to_string(),
            assertions: Vec::new(),
            mocks_in_file: vec!["./api".to_string()],
            imports_in_file: Vec::new(),
        }];
        let paths = collect_related_mock_paths(&owner, &tests);
        assert!(paths.is_empty());
    }

    #[test]
    fn collect_related_mock_paths_ignores_object_method_mentions() {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let tests = vec![TypeScriptTest {
            name: "unrelated method".to_string(),
            file: PathBuf::from("tests/cart.test.ts"),
            line: 1,
            body_text: "expect(order.applyDiscount(50)).toBe(40);".to_string(),
            assertions: Vec::new(),
            mocks_in_file: vec!["./api".to_string()],
            imports_in_file: Vec::new(),
        }];
        let paths = collect_related_mock_paths(&owner, &tests);
        assert!(paths.is_empty());
    }

    #[test]
    fn classify_change_surfaces_mocked_module_static_limit_in_missing_and_evidence()
    -> Result<(), String> {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
            owner_kind: OwnerKind::Function,
        };
        let tests = vec![TypeScriptTest {
            name: "alpha".to_string(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: "applyDiscount(50, 100)".to_string(),
            assertions: Vec::new(),
            mocks_in_file: vec!["./api".to_string()],
            imports_in_file: Vec::new(),
        }];
        let finding = classify_change(
            Path::new("src/lib.ts"),
            2,
            "    if (amount >= threshold) {",
            &[owner],
            &tests,
        )
        .ok_or_else(|| "expected a finding for the changed line".to_string())?;
        assert!(
            finding
                .missing
                .iter()
                .any(|line| line.contains("Static limit `mocked_module`")
                    && line.contains("./api"))
        );
        assert!(
            finding
                .evidence
                .iter()
                .any(|line| line.starts_with("static_limit mocked_module:"))
        );
        assert_eq!(
            finding.static_limit_kind,
            Some(StaticLimitKind::MockedModule)
        );
        Ok(())
    }
}
