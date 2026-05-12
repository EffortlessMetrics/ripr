//! TypeScript preview adapter — owner + test sub-slice.
//!
//! See `docs/specs/RIPR-SPEC-0027-typescript-preview-static-facts.md` and
//! `docs/adr/0008-typescript-parser-substrate.md`.
//!
//! This sub-slice extracts top-level function-declaration owners and
//! `test(...)` / `it(...)` blocks from TypeScript / JavaScript files,
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
//! Per-test-file recursion into nested `describe(...)` blocks and arrow
//! function owners is also deferred — the smallest useful fixture uses
//! `function name(...)` declarations and top-level `test(...)` calls.

use super::super::{AnalysisOptions, diff::ChangedFile};
use super::{LanguageAdapter, LanguageDiffResult, LanguageId, LanguageRepoResult, route};
use crate::config::OraclePolicy;
use crate::domain::{
    Confidence, DeltaKind, ExposureClass, Finding, LanguageId as DomainLanguageId, LanguageStatus,
    OracleKind, OracleStrength, Probe, ProbeFamily, ProbeId, RelatedTest, RevealEvidence,
    RiprEvidence, SourceLocation, StageEvidence, StageState,
};
use oxc_allocator::Allocator;
use oxc_ast::ast::{Expression, Statement};
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
/// Currently covers top-level `function name(...) { ... }` declarations
/// and their `export` wrappers. Arrow function consts, class methods,
/// and nested owners are deferred to follow-up issues.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptOwner {
    name: String,
    file: PathBuf,
    start_line: usize,
    end_line: usize,
}

/// Test block extracted from a TypeScript / JavaScript test file.
///
/// Currently covers top-level `test('name', fn)` and `it('name', fn)`
/// expression statements. Nested `describe(...)` recursion is deferred.
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
}

/// Assertion shape extracted from a single `expect(actual).matcher(...)`
/// chain inside a test body.
///
/// `matcher` is the canonical matcher name (`toBe`, `toEqual`, `toThrow`,
/// `toMatchSnapshot`, `toHaveBeenCalledWith`, ...). The full Jest/Vitest
/// matcher surface is large; this preview slice maps the most common
/// matchers to oracle vocabulary and tags the rest as `Unknown`.
/// Async-aware (`.resolves` / `.rejects`) chains and custom matchers
/// land in follow-up work covered by issue #767.
#[derive(Clone, Debug, PartialEq, Eq)]
struct TypeScriptAssertion {
    matcher: String,
    line: usize,
    oracle_kind: OracleKind,
    oracle_strength: OracleStrength,
}

fn oracle_for_matcher(matcher: &str) -> (OracleKind, OracleStrength) {
    match matcher {
        "toBe" | "toEqual" | "toStrictEqual" => (OracleKind::ExactValue, OracleStrength::Strong),
        "toThrow" | "toThrowError" => (OracleKind::ExactErrorVariant, OracleStrength::Strong),
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
        if let Some(owner) = owner_from_statement(stmt, file, source) {
            owners.push(owner);
        }
    }
    owners
}

fn owner_from_statement(
    stmt: &Statement<'_>,
    file: &Path,
    source: &str,
) -> Option<TypeScriptOwner> {
    if let Statement::FunctionDeclaration(func) = stmt
        && let Some(id) = &func.id
    {
        return Some(TypeScriptOwner {
            name: id.name.to_string(),
            file: file.to_path_buf(),
            start_line: line_for_offset(source, func.span.start as usize),
            end_line: line_for_offset(source, func.span.end as usize),
        });
    }
    if let Statement::ExportNamedDeclaration(export) = stmt
        && let Some(decl) = export.declaration.as_ref()
        && let oxc_ast::ast::Declaration::FunctionDeclaration(func) = decl
        && let Some(id) = &func.id
    {
        return Some(TypeScriptOwner {
            name: id.name.to_string(),
            file: file.to_path_buf(),
            start_line: line_for_offset(source, func.span.start as usize),
            end_line: line_for_offset(source, func.span.end as usize),
        });
    }
    None
}

fn extract_tests(file: &Path, source: &str) -> Vec<TypeScriptTest> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, source, source_type_for(file)).parse();
    if !ret.errors.is_empty() {
        return Vec::new();
    }
    let mocks = extract_mocks_from_statements(&ret.program.body);
    let mut tests = Vec::new();
    for stmt in &ret.program.body {
        if let Some(mut test) = test_from_statement(stmt, file, source) {
            test.mocks_in_file = mocks.clone();
            tests.push(test);
        }
    }
    tests
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

fn test_from_statement(stmt: &Statement<'_>, file: &Path, source: &str) -> Option<TypeScriptTest> {
    let Statement::ExpressionStatement(expr_stmt) = stmt else {
        return None;
    };
    let Expression::CallExpression(call) = &expr_stmt.expression else {
        return None;
    };
    let Expression::Identifier(ident) = &call.callee else {
        return None;
    };
    let callee_name = ident.name.as_str();
    if callee_name != "test" && callee_name != "it" {
        return None;
    }
    // First argument should be a string literal naming the test.
    let mut args = call.arguments.iter();
    let name_arg = args.next()?;
    let name = match name_arg {
        oxc_ast::ast::Argument::StringLiteral(literal) => literal.value.to_string(),
        _ => return None,
    };
    // Walk the second argument (the test body fn) for expect() chains.
    let assertions = match args.next() {
        Some(oxc_ast::ast::Argument::ArrowFunctionExpression(arrow)) => {
            collect_expect_assertions_in_statements(&arrow.body.statements, source)
        }
        Some(oxc_ast::ast::Argument::FunctionExpression(func)) => match &func.body {
            Some(body) => collect_expect_assertions_in_statements(&body.statements, source),
            None => Vec::new(),
        },
        _ => Vec::new(),
    };
    Some(TypeScriptTest {
        name,
        file: file.to_path_buf(),
        line: line_for_offset(source, call.span.start as usize),
        body_text: source[call.span.start as usize..call.span.end as usize].to_string(),
        assertions,
        // Populated by `extract_tests` (the only public extractor) once
        // per file before the test is returned to the caller.
        mocks_in_file: Vec::new(),
    })
}

/// Walk a list of statements (e.g., a function body) and collect every
/// `expect(actual).matcher(...)` expression statement we recognise.
fn collect_expect_assertions_in_statements(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
    source: &str,
) -> Vec<TypeScriptAssertion> {
    let mut out = Vec::new();
    for stmt in statements {
        if let Statement::ExpressionStatement(expr_stmt) = stmt
            && let Some(assertion) = expect_assertion_from_expression(&expr_stmt.expression, source)
        {
            out.push(assertion);
        }
    }
    out
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
        line: line_for_offset(source, outer_call.span.start as usize),
        oracle_kind,
        oracle_strength,
    })
}

fn find_related_tests(owner: &TypeScriptOwner, all_tests: &[TypeScriptTest]) -> Vec<RelatedTest> {
    let needle = format!("{}(", owner.name);
    all_tests
        .iter()
        .filter(|test| test.body_text.contains(&needle))
        .map(|test| {
            let strongest = strongest_assertion(&test.assertions);
            let (oracle_kind, oracle_strength, oracle_text) = match strongest {
                Some(assertion) => (
                    assertion.oracle_kind.clone(),
                    assertion.oracle_strength.clone(),
                    Some(format!("expect(...).{}(...)", assertion.matcher)),
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
    let needle = format!("{}(", owner.name);
    let mut paths: Vec<String> = Vec::new();
    for test in all_tests
        .iter()
        .filter(|test| test.body_text.contains(&needle))
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
            vec![format!(
                "No test references `{}(` — add a test that calls the changed owner.",
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
                "Related test reaches `{}` but the strongest extracted oracle is `{}`; upgrade by adding an exact-value (`toBe` / `toEqual` / `toStrictEqual`) or exact-error-variant (`toThrow`) assertion.",
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
        "TypeScript preview adapter found no strong discriminator; upgrade an assertion to `toBe` / `toEqual` / `toStrictEqual` / `toThrow` to escalate.".to_string()
    };
    let discriminate =
        StageEvidence::new(discriminate_state, Confidence::Low, &discriminate_summary);

    let recommended = match &class {
        ExposureClass::Exposed => {
            "TypeScript preview: changed behavior is observed under a strong oracle; verify the assertion targets the changed boundary value.".to_string()
        }
        ExposureClass::NoStaticPath => {
            "TypeScript preview: no test references the changed owner; add a test that calls the owner and asserts the changed behavior with `toBe` / `toEqual`.".to_string()
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
        language: Some(DomainLanguageId::TypeScript),
        language_status: Some(LanguageStatus::Preview),
    })
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
    }

    #[test]
    fn extract_owners_recognizes_exported_function() {
        let owners = extract_owners(
            Path::new("src/lib.ts"),
            "export function publicHelper(): void {}\n",
        );
        assert_eq!(owners.len(), 1);
        assert_eq!(owners[0].name, "publicHelper");
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
            },
            TypeScriptTest {
                name: "unrelated".to_string(),
                file: PathBuf::from("tests/other.test.ts"),
                line: 1,
                body_text: r#"test("unrelated", () => { expect(otherHelper()).toBe(true); });"#
                    .to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
            },
        ];
        let related = find_related_tests(&owner, &tests);
        assert_eq!(related.len(), 1);
        assert_eq!(related[0].name, "alpha");
    }

    #[test]
    fn classify_change_returns_weakly_exposed_when_related_test_exists() -> Result<(), String> {
        let owner = TypeScriptOwner {
            name: "applyDiscount".to_string(),
            file: PathBuf::from("src/lib.ts"),
            start_line: 1,
            end_line: 5,
        };
        let test = TypeScriptTest {
            name: "alpha".to_string(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: "applyDiscount(50, 100)".to_string(),
            assertions: Vec::new(),
            mocks_in_file: Vec::new(),
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
    fn classify_change_matches_owner_file_before_line_range() -> Result<(), String> {
        let owners = vec![
            TypeScriptOwner {
                name: "alphaScore".to_string(),
                file: PathBuf::from("src/a.ts"),
                start_line: 1,
                end_line: 5,
            },
            TypeScriptOwner {
                name: "betaScore".to_string(),
                file: PathBuf::from("src/b.ts"),
                start_line: 1,
                end_line: 5,
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
            },
            TypeScriptTest {
                name: "beta keeps its threshold".to_string(),
                file: PathBuf::from("tests/b.test.ts"),
                line: 1,
                body_text: "expect(betaScore(12)).toBe(13);".to_string(),
                assertions: Vec::new(),
                mocks_in_file: Vec::new(),
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
    fn extract_tests_recognizes_resolves_async_chain() {
        let tests = extract_tests(
            Path::new("tests/lib.test.ts"),
            r#"test("async", async () => {
    await expect(loader()).resolves.toBe(42);
});
"#,
        );
        assert_eq!(tests.len(), 1);
        // The async chain is one level deeper; current scaffold matches
        // only top-level expect().matcher() shapes inside the test body.
        // The async `expect(...).resolves.toBe(...)` lives inside an
        // `await` expression, which is not a top-level expression
        // statement we walk yet (deferred to #767 follow-up). The test
        // pins this current limit so a future change tightens it.
        assert!(tests[0].assertions.is_empty());
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
            (OracleKind::ExactErrorVariant, OracleStrength::Strong)
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
        };
        let test = TypeScriptTest {
            name: "alpha".to_string(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: "applyDiscount(50, 100)".to_string(),
            assertions: vec![TypeScriptAssertion {
                matcher: "toBe".to_string(),
                line: 2,
                oracle_kind: OracleKind::ExactValue,
                oracle_strength: OracleStrength::Strong,
            }],
            mocks_in_file: Vec::new(),
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
        };
        let tests = vec![
            TypeScriptTest {
                name: "alpha".to_string(),
                file: PathBuf::from("tests/lib.test.ts"),
                line: 1,
                body_text: "applyDiscount(1, 2)".to_string(),
                assertions: Vec::new(),
                mocks_in_file: vec!["./api".to_string()],
            },
            TypeScriptTest {
                name: "beta".to_string(),
                file: PathBuf::from("tests/lib.test.ts"),
                line: 2,
                body_text: "applyDiscount(3, 4)".to_string(),
                assertions: Vec::new(),
                mocks_in_file: vec!["./api".to_string()],
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
        };
        let tests = vec![TypeScriptTest {
            name: "unrelated".to_string(),
            file: PathBuf::from("tests/other.test.ts"),
            line: 1,
            body_text: "otherHelper()".to_string(),
            assertions: Vec::new(),
            mocks_in_file: vec!["./api".to_string()],
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
        };
        let tests = vec![TypeScriptTest {
            name: "alpha".to_string(),
            file: PathBuf::from("tests/lib.test.ts"),
            line: 1,
            body_text: "applyDiscount(50, 100)".to_string(),
            assertions: Vec::new(),
            mocks_in_file: vec!["./api".to_string()],
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
        Ok(())
    }
}
