//! Test extraction for the TypeScript preview adapter.

use super::*;

pub(crate) fn extract_tests(file: &Path, source: &str) -> Vec<TypeScriptTest> {
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

/// Walk a list of top-level statements and collect every syntactic
/// `vi.mock("path")` / `jest.mock("path")` argument we see. The list is
/// deduplicated and used by the classifier to surface the
/// `mocked_module` static-limit per RIPR-SPEC-0026.
///
/// This is purely syntactic — the adapter does not resolve the mocked
/// module identifier through the project's import graph, so the limit
/// surfaces exactly when the test file contains the mock call shape.
pub(crate) fn extract_mocks_from_statements(
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

pub(crate) fn collect_tests_from_statements(
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

pub(crate) fn describe_body_from_statement<'a>(
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

pub(crate) fn test_from_statement(
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
        local_name: name,
        describe_names: describe_stack.to_vec(),
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

pub(crate) fn test_name_and_assertions_from_call(
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

pub(crate) fn test_callee_is_identifier(call: &oxc_ast::ast::CallExpression<'_>) -> bool {
    let Expression::Identifier(ident) = &call.callee else {
        return false;
    };
    matches!(ident.name.as_str(), "test" | "it")
}

pub(crate) fn test_callee_is_each(call: &oxc_ast::ast::CallExpression<'_>) -> bool {
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

pub(crate) fn string_argument(arg: &oxc_ast::ast::Argument<'_>) -> Option<String> {
    match arg {
        oxc_ast::ast::Argument::StringLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

pub(crate) fn function_body_statements_from_argument<'a>(
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

pub(crate) fn qualified_test_name(describe_stack: &[String], name: &str) -> String {
    if describe_stack.is_empty() {
        return name.to_string();
    }
    let mut parts = describe_stack.to_vec();
    parts.push(name.to_string());
    parts.join(" ")
}
