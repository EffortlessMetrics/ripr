//! Test extraction for the TypeScript preview adapter.

use super::*;

#[derive(Clone, Copy)]
enum TestDeclarationRoot {
    Test,
    Describe,
}

impl TestDeclarationRoot {
    fn matches_identifier(self, name: &str) -> bool {
        match self {
            Self::Test => matches!(name, "test" | "it"),
            Self::Describe => name == "describe",
        }
    }
}

pub(crate) fn extract_tests(file: &Path, source: &str) -> Vec<TypeScriptTest> {
    // Same guard as owner extraction (#4101): a file under the nesting
    // budget can still overflow the caller stack, so the second parse must
    // not run on the main thread.
    let Ok(tests) = parse_on_worker(file, source, |file, source, allocator| {
        let ret = Parser::new(allocator, source, source_type_for(file)).parse();
        if !ret.errors.is_empty() {
            return Vec::new();
        }
        let imports = extract_imports_from_statements(&ret.program.body);
        let mocks = extract_mocks_from_statements(&ret.program.body, &imports);
        let mut tests = Vec::new();
        let mut scope = TestScope::default();
        // One line index per source; every test and assertion line is a
        // binary search against it rather than a rescan from byte 0.
        collect_tests_from_statements(
            &ret.program.body,
            file,
            &SourceText::new(source),
            &mocks,
            &imports,
            &mut scope,
            &mut tests,
        );
        // Comments, module specifiers and call-statement name literals are
        // the only text the rebinding check skips, each at its exact AST
        // span; every other string, template and regex stays visible, so no
        // mis-lexed quote can hide a write.
        let masked: Vec<std::ops::Range<usize>> = ret
            .program
            .comments
            .iter()
            .map(|comment| comment.span)
            .chain(ret.program.body.iter().filter_map(module_specifier_span))
            .map(|span| span.start as usize..span.end as usize)
            .chain(scope.names.iter().cloned())
            .collect();
        withhold_rebound_scope_receivers(source, &masked, &scope.sites, &mut tests);
        tests
    }) else {
        return Vec::new();
    };
    tests
}

/// Walk a list of statements and collect every syntactic owner-module mock
/// registration (`vi.mock("path")`, `jest.mock("path")`, the non-hoisted
/// `doMock`, Jest's `unstable_mockModule`/`setMock`, and `mock.module` from
/// `bun:test`/`node:test`) at ANY statement depth the runners hoist through —
/// including `describe(...)` callback bodies, where both Jest and Vitest
/// legally allow `mock`/`doMock` calls. The list is deduplicated and used by
/// the classifier to surface the `mocked_module` static-limit per
/// RIPR-SPEC-0026, and by the owner-module mock guard.
///
/// The runner object may be `vi`/`jest`, a renamed or namespace import of
/// either from a test runner module, or a `const` alias or destructured
/// method bound anywhere in the file (#4294). Aliases ignore lexical scope,
/// which only errs toward recording a mock. Parentheses,
/// type assertions, optional calls and `await` around the call are seen
/// through. A specifier the adapter cannot read is recorded as
/// [`UNRESOLVED_MOCK_SPECIFIER`] so the guard fails closed.
///
/// This is purely syntactic — the adapter does not resolve the mocked
/// module identifier through the project's import graph, so the limit
/// surfaces exactly when the test file contains the mock call shape.
pub(crate) fn extract_mocks_from_statements(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
    imports: &[TypeScriptImport],
) -> Vec<String> {
    let mut runner = MockRunner::from_imports(imports);
    // A callback or hoisted function may use an alias declared after it
    // (`beforeEach(() => m.mock(...)); const m = vi;`), so a first walk binds
    // every alias in the file and the second collects with all of them.
    collect_mock_paths(&mut runner, statements, &mut Vec::new());
    let mut out: Vec<String> = Vec::new();
    collect_mock_paths(&mut runner, statements, &mut out);
    out
}

/// Recorded in place of a mock specifier that is not a string: a variable,
/// a template with substitutions, a spread, or `import(expr)`. The mocked
/// module is unknown, so the owner-module guard treats it as the owner's
/// (#4294).
pub(crate) const UNRESOLVED_MOCK_SPECIFIER: &str = "<unresolved mock specifier>";

const RUNNER_OBJECTS: [&str; 2] = ["vi", "jest"];

/// Runner-object methods that replace a module for the importing test.
const RUNNER_MOCK_METHODS: [&str; 4] = ["mock", "doMock", "unstable_mockModule", "setMock"];

/// Runner modules whose `mock` export registers a module mock through
/// `mock.module(path, ...)`.
const MOCK_MODULE_RUNNERS: [&str; 2] = ["bun:test", "node:test"];

/// The names through which a test file reaches a runner's mock API.
#[derive(Default)]
struct MockRunner {
    /// Identifiers bound to the runner object (`vi`, `jest`, aliases).
    objects: Vec<String>,
    /// Namespace imports of a runner module (`import * as vt from "vitest"`).
    namespaces: Vec<String>,
    /// Identifiers bound to a runner mock method (`const { mock } = vi`).
    functions: Vec<String>,
    /// Identifiers bound to a `mock` export with a `module` method.
    module_mockers: Vec<String>,
}

impl MockRunner {
    fn from_imports(imports: &[TypeScriptImport]) -> Self {
        let mut runner = Self {
            objects: RUNNER_OBJECTS.iter().map(ToString::to_string).collect(),
            ..Self::default()
        };
        for import in imports {
            let source = import.source.as_str();
            if !TEST_RUNNER_MODULES.contains(&source) {
                continue;
            }
            let imported = import.imported.as_deref();
            if import.namespace {
                runner.namespaces.push(import.local.clone());
            } else if imported.is_some_and(|name| RUNNER_OBJECTS.contains(&name)) {
                runner.objects.push(import.local.clone());
            } else if imported == Some("mock") && MOCK_MODULE_RUNNERS.contains(&source) {
                runner.module_mockers.push(import.local.clone());
            }
        }
        runner
    }

    /// Record the names a declarator binds to the runner: `const m = vi`,
    /// `const { mock: m } = vi`, `const { vi: v } = vt`.
    fn bind(&mut self, declarator: &oxc_ast::ast::VariableDeclarator<'_>) {
        let Some(init) = &declarator.init else {
            return;
        };
        match &declarator.id {
            oxc_ast::ast::BindingPattern::BindingIdentifier(id) if self.is_object(init) => {
                self.objects.push(id.name.to_string());
            }
            oxc_ast::ast::BindingPattern::BindingIdentifier(id) if self.is_mock_method(init) => {
                self.functions.push(id.name.to_string());
            }
            oxc_ast::ast::BindingPattern::BindingIdentifier(id) if self.is_module_mocker(init) => {
                self.module_mockers.push(id.name.to_string());
            }
            oxc_ast::ast::BindingPattern::ObjectPattern(pattern) => {
                let from_object = self.is_object(init);
                let from_namespace = self.is_namespace(init);
                for property in &pattern.properties {
                    // `{ mock = fallback }` binds `mock` through a default.
                    let value = match &property.value {
                        oxc_ast::ast::BindingPattern::AssignmentPattern(assign) => &assign.left,
                        value => value,
                    };
                    let (Some(key), oxc_ast::ast::BindingPattern::BindingIdentifier(local)) =
                        (property.key.static_name(), value)
                    else {
                        continue;
                    };
                    let local = local.name.to_string();
                    if from_object && RUNNER_MOCK_METHODS.contains(&key.as_ref()) {
                        self.functions.push(local);
                    } else if from_namespace && RUNNER_OBJECTS.contains(&key.as_ref()) {
                        self.objects.push(local);
                    } else if from_namespace && key == "mock" {
                        self.module_mockers.push(local);
                    }
                }
            }
            _ => {}
        }
    }

    fn is_namespace(&self, expression: &Expression<'_>) -> bool {
        matches!(expression.get_inner_expression(), Expression::Identifier(ident)
            if self.namespaces.iter().any(|name| name == ident.name.as_str()))
    }

    fn is_object(&self, expression: &Expression<'_>) -> bool {
        let expression = expression.get_inner_expression();
        if let Expression::Identifier(ident) = expression {
            return self.objects.iter().any(|name| name == ident.name.as_str());
        }
        member_parts(expression).is_some_and(|(object, property)| {
            RUNNER_OBJECTS.contains(&property) && self.is_namespace(object)
        })
    }

    /// `true` for `vi.mock` (or another runner mock method) read as a value,
    /// possibly through `.bind(...)`: `const doMock = vi.doMock`.
    fn is_mock_method(&self, expression: &Expression<'_>) -> bool {
        let expression = expression.get_inner_expression();
        if let Expression::CallExpression(call) = expression
            && let Some((bound, "bind")) = member_parts(call.callee.get_inner_expression())
        {
            return self.is_mock_method(bound);
        }
        member_parts(expression).is_some_and(|(object, property)| {
            RUNNER_MOCK_METHODS.contains(&property) && self.is_object(object)
        })
    }

    /// `true` for the `mock` export of `bun:test`/`node:test` (whose
    /// `module` method registers a module mock), imported, aliased, or read
    /// from a namespace import (`node.mock`).
    fn is_module_mocker(&self, expression: &Expression<'_>) -> bool {
        let expression = expression.get_inner_expression();
        if let Expression::Identifier(ident) = expression {
            return self
                .module_mockers
                .iter()
                .any(|name| name == ident.name.as_str());
        }
        member_parts(expression)
            .is_some_and(|(object, property)| property == "mock" && self.is_namespace(object))
    }

    /// `true` when calling `callee` registers a module mock.
    fn is_mock_callee(&self, callee: &Expression<'_>) -> bool {
        let mut callee = callee.get_inner_expression();
        // `(0, vi.mock)(...)` calls the last operand.
        while let Expression::SequenceExpression(sequence) = callee {
            let Some(last) = sequence.expressions.last() else {
                return false;
            };
            callee = last.get_inner_expression();
        }
        if let Expression::Identifier(ident) = callee {
            return self
                .functions
                .iter()
                .any(|name| name == ident.name.as_str());
        }
        self.is_mock_method(callee)
            || member_parts(callee).is_some_and(|(object, property)| {
                property == "module" && self.is_module_mocker(object)
            })
    }
}

/// The object and static property name of `a.b`, `a["b"]` or ``a[`b`]``.
fn member_parts<'e, 'a>(expression: &'e Expression<'a>) -> Option<(&'e Expression<'a>, &'a str)> {
    match expression {
        Expression::StaticMemberExpression(member) => {
            Some((&member.object, member.property.name.as_str()))
        }
        Expression::ComputedMemberExpression(member) => member
            .static_property_name()
            .map(|name| (&member.object, name.as_str())),
        _ => None,
    }
}

/// Recursively collect mock paths from statements. Statement containers
/// (blocks, if/else, loops, try/catch, switch, exports) and function bodies
/// (declarations, callbacks, arrow initializers) are walked; everything else
/// is ignored. Runner aliases are bound as their declarations are reached.
/// Deduplication preserves first-seen order.
fn collect_mock_paths(
    runner: &mut MockRunner,
    statements: &[Statement<'_>],
    out: &mut Vec<String>,
) {
    for stmt in statements {
        match stmt {
            Statement::BlockStatement(block) => collect_mock_paths(runner, &block.body, out),
            Statement::ExpressionStatement(expr_stmt) => {
                collect_mock_path_from_expression(runner, &expr_stmt.expression, out);
            }
            Statement::IfStatement(if_stmt) => {
                collect_mock_paths(runner, std::slice::from_ref(&if_stmt.consequent), out);
                if let Some(alternate) = &if_stmt.alternate {
                    collect_mock_paths(runner, std::slice::from_ref(alternate), out);
                }
            }
            Statement::DoWhileStatement(do_while) => {
                collect_mock_paths(runner, std::slice::from_ref(&do_while.body), out)
            }
            Statement::WhileStatement(while_stmt) => {
                collect_mock_paths(runner, std::slice::from_ref(&while_stmt.body), out)
            }
            Statement::ForStatement(for_stmt) => {
                collect_mock_paths(runner, std::slice::from_ref(&for_stmt.body), out)
            }
            Statement::ForInStatement(for_in) => {
                collect_mock_paths(runner, std::slice::from_ref(&for_in.body), out)
            }
            Statement::ForOfStatement(for_of) => {
                collect_mock_paths(runner, std::slice::from_ref(&for_of.body), out)
            }
            Statement::LabeledStatement(labeled) => {
                collect_mock_paths(runner, std::slice::from_ref(&labeled.body), out)
            }
            Statement::TryStatement(try_stmt) => {
                collect_mock_paths(runner, &try_stmt.block.body, out);
                if let Some(handler) = &try_stmt.handler {
                    collect_mock_paths(runner, &handler.body.body, out);
                }
                if let Some(finalizer) = &try_stmt.finalizer {
                    collect_mock_paths(runner, &finalizer.body, out);
                }
            }
            Statement::SwitchStatement(switch_stmt) => {
                for case in &switch_stmt.cases {
                    collect_mock_paths(runner, &case.consequent, out);
                }
            }
            Statement::VariableDeclaration(decl) => {
                collect_mock_paths_from_variables(runner, decl, out)
            }
            Statement::FunctionDeclaration(func) => {
                if let Some(body) = &func.body {
                    collect_mock_paths(runner, &body.statements, out);
                }
            }
            Statement::ReturnStatement(ret) => {
                if let Some(argument) = &ret.argument {
                    collect_mock_path_from_expression(runner, argument, out);
                }
            }
            Statement::ClassDeclaration(class) => collect_mock_paths_from_class(runner, class, out),
            Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                oxc_ast::ast::ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                    if let Some(body) = &func.body {
                        collect_mock_paths(runner, &body.statements, out);
                    }
                }
                oxc_ast::ast::ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                    collect_mock_paths_from_class(runner, class, out)
                }
                other => {
                    if let Some(expression) = other.as_expression() {
                        collect_mock_path_from_expression(runner, expression, out);
                    }
                }
            },
            Statement::ExportNamedDeclaration(export) => match &export.declaration {
                Some(oxc_ast::ast::Declaration::VariableDeclaration(decl)) => {
                    collect_mock_paths_from_variables(runner, decl, out)
                }
                Some(oxc_ast::ast::Declaration::FunctionDeclaration(func)) => {
                    if let Some(body) = &func.body {
                        collect_mock_paths(runner, &body.statements, out);
                    }
                }
                Some(oxc_ast::ast::Declaration::ClassDeclaration(class)) => {
                    collect_mock_paths_from_class(runner, class, out)
                }
                _ => {}
            },
            _ => {}
        }
    }
}

/// Class static blocks, property initializers and method bodies run test
/// code too.
fn collect_mock_paths_from_class(
    runner: &mut MockRunner,
    class: &oxc_ast::ast::Class<'_>,
    out: &mut Vec<String>,
) {
    for element in &class.body.body {
        match element {
            oxc_ast::ast::ClassElement::StaticBlock(block) => {
                collect_mock_paths(runner, &block.body, out)
            }
            oxc_ast::ast::ClassElement::MethodDefinition(method) => {
                if let Some(body) = &method.value.body {
                    collect_mock_paths(runner, &body.statements, out);
                }
            }
            oxc_ast::ast::ClassElement::PropertyDefinition(property) => {
                if let Some(value) = &property.value {
                    collect_mock_path_from_expression(runner, value, out);
                }
            }
            _ => {}
        }
    }
}

fn collect_mock_paths_from_variables(
    runner: &mut MockRunner,
    decl: &oxc_ast::ast::VariableDeclaration<'_>,
    out: &mut Vec<String>,
) {
    for declarator in &decl.declarations {
        runner.bind(declarator);
        if let Some(init) = &declarator.init {
            collect_mock_path_from_expression(runner, init, out);
        }
    }
}

/// Collect a mock path from an expression: a direct mock call, possibly
/// parenthesized, awaited, optional or in a sequence, or a call or function
/// whose body may register one (so a describe-scoped or helper-built mock is
/// found). A mock call chained after another call (`jest.mock("a").mock("b")`)
/// is found by descending into a member callee's object, so chained
/// registrations stay under the owner-module mock guard.
fn collect_mock_path_from_expression(
    runner: &mut MockRunner,
    expression: &Expression<'_>,
    out: &mut Vec<String>,
) {
    let call = match expression.get_inner_expression() {
        Expression::CallExpression(call) => call,
        Expression::ChainExpression(chain) => {
            let oxc_ast::ast::ChainElement::CallExpression(call) = &chain.expression else {
                return;
            };
            call
        }
        Expression::AwaitExpression(await_expr) => {
            return collect_mock_path_from_expression(runner, &await_expr.argument, out);
        }
        Expression::SequenceExpression(sequence) => {
            for item in &sequence.expressions {
                collect_mock_path_from_expression(runner, item, out);
            }
            return;
        }
        Expression::ArrowFunctionExpression(arrow) => {
            return collect_mock_paths(runner, &arrow.body.statements, out);
        }
        Expression::FunctionExpression(func) => {
            if let Some(body) = &func.body {
                collect_mock_paths(runner, &body.statements, out);
            }
            return;
        }
        Expression::UnaryExpression(unary) => {
            return collect_mock_path_from_expression(runner, &unary.argument, out);
        }
        Expression::LogicalExpression(logical) => {
            collect_mock_path_from_expression(runner, &logical.left, out);
            return collect_mock_path_from_expression(runner, &logical.right, out);
        }
        Expression::ConditionalExpression(conditional) => {
            collect_mock_path_from_expression(runner, &conditional.consequent, out);
            return collect_mock_path_from_expression(runner, &conditional.alternate, out);
        }
        Expression::AssignmentExpression(assign) => {
            // `m = vi` binds an alias like a declaration does.
            if let oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(target) = &assign.left
                && runner.is_object(&assign.right)
            {
                runner.objects.push(target.name.to_string());
            }
            return collect_mock_path_from_expression(runner, &assign.right, out);
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                if let Some(element) = element.as_expression() {
                    collect_mock_path_from_expression(runner, element, out);
                }
            }
            return;
        }
        Expression::ObjectExpression(object) => {
            for property in &object.properties {
                if let oxc_ast::ast::ObjectPropertyKind::ObjectProperty(property) = property {
                    collect_mock_path_from_expression(runner, &property.value, out);
                }
            }
            return;
        }
        _ => return,
    };
    if let Some(path) = mock_path_from_call(runner, call)
        && !out.iter().any(|existing| existing == &path)
    {
        out.push(path);
    }
    if let Expression::StaticMemberExpression(member) = &call.callee {
        collect_mock_path_from_expression(runner, &member.object, out);
    }
    // Callbacks, including ones wrapped in another call
    // (`beforeEach(wrap(() => ...))`), may register a mock.
    for argument in &call.arguments {
        if let Some(argument) = argument.as_expression() {
            collect_mock_path_from_expression(runner, argument, out);
        }
    }
}

/// Extract the mocked module path from a runner mock call (see
/// [`extract_mocks_from_statements`] for the recognised callees). `doMock`
/// is the non-hoisted variant both runners expose (it affects only modules
/// loaded after the call); the adapter cannot prove the observed call
/// reaches the real module, so the owner-module mock guard treats it
/// exactly like `mock` (#4103 shape 2).
///
/// The specifier may be a string, a substitution-free template, Vitest's
/// typed `import("path")` form or Jest's `require.resolve("path")`, seen
/// through parentheses and type assertions; anything else yields
/// [`UNRESOLVED_MOCK_SPECIFIER`] (#4294).
fn mock_path_from_call(
    runner: &MockRunner,
    call: &oxc_ast::ast::CallExpression<'_>,
) -> Option<String> {
    if !runner.is_mock_callee(&call.callee) {
        return None;
    }
    // A spread first argument (`vi.mock(...args)`) is as opaque as a variable.
    let specifier = call
        .arguments
        .first()?
        .as_expression()
        .and_then(|argument| match argument.get_inner_expression() {
            Expression::ImportExpression(import) => string_value(&import.source),
            Expression::CallExpression(resolve)
                if member_parts(&resolve.callee).is_some_and(|(object, property)| {
                    property == "resolve"
                        && matches!(object, Expression::Identifier(ident) if ident.name == "require")
                }) =>
            {
                resolve
                    .arguments
                    .first()
                    .and_then(oxc_ast::ast::Argument::as_expression)
                    .and_then(string_value)
            }
            other => string_value(other),
        });
    Some(specifier.unwrap_or_else(|| UNRESOLVED_MOCK_SPECIFIER.to_string()))
}

/// The value of a string literal or a template without substitutions.
fn string_value(expression: &Expression<'_>) -> Option<String> {
    match expression.get_inner_expression() {
        Expression::StringLiteral(literal) => Some(literal.value.to_string()),
        Expression::TemplateLiteral(template) => {
            template.single_quasi().map(|quasi| quasi.to_string())
        }
        _ => None,
    }
}

pub(crate) fn collect_tests_from_statements(
    statements: &[Statement<'_>],
    file: &Path,
    source: &SourceText<'_>,
    mocks: &[String],
    imports: &[TypeScriptImport],
    scope: &mut TestScope,
    tests: &mut Vec<TypeScriptTest>,
) {
    // Bindings that hold for every test in this scope: variable, function and
    // class declarations and `beforeEach`/`beforeAll` assignments at this
    // level. Collected before any test so a hook written after a test still
    // counts, as it does at runtime.
    let mut level = Vec::new();
    let mut sites = Vec::new();
    let mut iterables = Vec::new();
    let mut returned = false;
    for stmt in statements {
        if !returned {
            collect_const_iterables(stmt, &mut iterables);
        }
        let start = level.len();
        collect_scope_bindings(stmt, source, &mut level, &mut sites);
        // After a possible early `return`, a binding may never run.
        if returned {
            level[start..]
                .iter_mut()
                .for_each(|entry| entry.1 = ScopeValue::Other);
        }
        returned |= may_return(stmt, source);
    }
    scope.sites.extend(sites);
    // A `beforeEach`/`beforeAll` the file declares, or imports from anything
    // but a test runner, is not known to run before each test: its writes are
    // only ambiguous.
    for (hook, phase) in [
        ("beforeAll", Phase::BeforeAll),
        ("beforeEach", Phase::BeforeEach),
    ] {
        let shadowed = scope
            .levels
            .iter()
            .chain(std::iter::once(&level))
            .flatten()
            .any(|(name, _, entry_phase)| name == hook && *entry_phase == Phase::Declaration)
            || imports.iter().any(|import| {
                import.local == hook
                    && (!TEST_RUNNER_MODULES.contains(&import.source.as_str())
                        || import.imported.as_deref() != Some(hook))
            });
        if shadowed {
            for entry in level
                .iter_mut()
                .filter(|(_, _, entry_phase)| *entry_phase == phase)
            {
                entry.1 = ScopeValue::Other;
                entry.2 = Phase::Interleaved;
            }
        }
    }
    scope.levels.push(level);
    scope.iterables.push(iterables);
    for stmt in statements {
        if let Some(span) = name_literal_span(stmt) {
            scope.names.push(span);
        }
        if let Some((describe_name, body)) = describe_body_from_statement(stmt, source) {
            // `describe.each(...)('x', (cart) => ...)` binds its parameters
            // for every test inside.
            scope.levels.push(
                statement_callback_parameter_names(stmt, 1)
                    .into_iter()
                    .map(|name| (name, ScopeValue::Other, Phase::Declaration))
                    .collect(),
            );
            scope.iterables.push(Vec::new());
            scope.describe_names.push(describe_name);
            collect_tests_from_statements(body, file, source, mocks, imports, scope, tests);
            scope.describe_names.pop();
            scope.iterables.pop();
            scope.levels.pop();
            continue;
        }
        if let Some((loop_bindings, body)) = loop_body_from_statement(stmt, scope) {
            // A `for`/`for...of`/`for...in` or `.forEach` callback body
            // registers its tests once per iteration; its loop variables or
            // callback parameters shadow every enclosing binding.
            scope.levels.push(
                loop_bindings
                    .into_iter()
                    .map(|name| (name, ScopeValue::Other, Phase::Declaration))
                    .collect(),
            );
            scope.iterables.push(Vec::new());
            collect_tests_from_statements(body, file, source, mocks, imports, scope, tests);
            scope.iterables.pop();
            scope.levels.pop();
            continue;
        }
        if let Some(mut test) = test_from_statement(stmt, file, source, &scope.describe_names) {
            test.mocks_in_file = mocks.to_vec();
            test.imports_in_file = imports.to_vec();
            // Test callback parameters (`it.each` rows, Vitest fixtures)
            // shadow every enclosing binding of the same name.
            let parameters = statement_callback_parameter_names(stmt, 1)
                .into_iter()
                .map(|name| (name, ScopeValue::Other, Phase::Declaration))
                .collect();
            test.scope_bindings = scope.resolve_with(parameters);
            tests.push(test);
        }
    }
    scope.iterables.pop();
    scope.levels.pop();
}

/// Modules whose `beforeEach`/`beforeAll` run before every test in scope.
const TEST_RUNNER_MODULES: [&str; 4] = ["vitest", "@jest/globals", "bun:test", "node:test"];

/// The enclosing scopes of the statements being walked.
#[derive(Default)]
pub(crate) struct TestScope {
    /// Describe names, outermost first.
    describe_names: Vec<String>,
    /// Name bindings of the file and each enclosing describe, outermost first.
    levels: Vec<Vec<ScopeEntry>>,
    /// Per entry of `levels`: the `const` names that level binds to a
    /// non-empty array or object literal, so a loop over one of them is
    /// known to register its tests at least once.
    iterables: Vec<Vec<(String, IterableLiteral)>>,
    /// Source offsets of every declaration and hook write the walk recorded.
    sites: Vec<usize>,
    /// Spans of string literals passed first to a call statement: describe
    /// and test names, `vi.mock('../src/cart')` paths.
    names: Vec<std::ops::Range<usize>>,
}

/// One binding a scope-level statement makes, and when it runs.
type ScopeEntry = (String, ScopeValue, Phase);

/// When a scope-level write runs relative to a test in that scope, in order:
/// declarations while the file or `describe` callback runs, then `beforeAll`
/// hooks, then writes that may run between tests (another test, an
/// `afterEach` hook, a nested `describe`), then `beforeEach` hooks, which
/// reset the name before every test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Phase {
    Declaration,
    BeforeAll,
    Interleaved,
    BeforeEach,
}

/// What one setup statement binds a name to.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ScopeValue {
    /// `let cart: Cart;`: declared here, assigned elsewhere or never.
    Declared,
    /// `new Cart()` / `new shop.Cart()`, by constructor text.
    Constructed(String),
    /// Anything else, including a shadowing function, class or parameter.
    Other,
}

impl TestScope {
    /// Resolve every bound name to its innermost scope. Within that scope the
    /// last hook write decides, since hooks run after the declarations and
    /// before every test; a `beforeAll` write is ambiguous when an enclosing
    /// `beforeEach` also writes the name. Without a hook write the name is constructed only
    /// when every declaration of it constructs the same class; a declaration
    /// without an initializer is neutral.
    fn resolve_with(&self, innermost: Vec<ScopeEntry>) -> Vec<TypeScriptScopeBinding> {
        let mut resolved: Vec<TypeScriptScopeBinding> = Vec::new();
        let levels: Vec<&Vec<ScopeEntry>> = std::iter::once(&innermost)
            .chain(self.levels.iter().rev())
            .collect();
        for (depth, level) in levels.iter().enumerate() {
            let mut names: Vec<&str> = level.iter().map(|(name, _, _)| name.as_str()).collect();
            names.sort_unstable();
            names.dedup();
            for name in names {
                if resolved.iter().any(|binding| binding.name == name) {
                    continue;
                }
                let mut writes: Vec<(&ScopeValue, Phase)> = level
                    .iter()
                    .filter(|(bound, _, _)| bound == name)
                    .map(|(_, value, phase)| (value, *phase))
                    .collect();
                // Stable: source order is kept within a phase.
                writes.sort_by_key(|(_, phase)| *phase);
                let constructed_by = match writes.last() {
                    // An enclosing `beforeEach` runs after this `beforeAll`.
                    Some((_, Phase::BeforeAll))
                        if levels[depth + 1..].iter().any(|outer| {
                            outer.iter().any(|(bound, _, phase)| {
                                bound == name && *phase == Phase::BeforeEach
                            })
                        }) =>
                    {
                        None
                    }
                    Some((value, phase)) if *phase != Phase::Declaration => match value {
                        ScopeValue::Constructed(constructor) => Some(constructor.clone()),
                        ScopeValue::Declared | ScopeValue::Other => None,
                    },
                    _ => {
                        let mut constructors = writes.iter().filter_map(|(value, _)| match value {
                            ScopeValue::Declared => None,
                            ScopeValue::Constructed(constructor) => Some(Some(constructor)),
                            ScopeValue::Other => Some(None),
                        });
                        let first = constructors.next().flatten();
                        first
                            .filter(|first| constructors.all(|next| next == Some(*first)))
                            .cloned()
                    }
                };
                resolved.push(TypeScriptScopeBinding {
                    name: name.to_string(),
                    constructed_by,
                    // `levels` is the test's own parameters, then the
                    // enclosing levels innermost first; the last is the file.
                    file_level: depth > 0 && depth == levels.len() - 1,
                });
            }
        }
        resolved
    }
}

/// Record the names one scope-level statement binds for every test in the
/// scope: declarations, assignments made by a `beforeEach`/`beforeAll` hook,
/// and, as ambiguous, writes made by any other callback at this level (a
/// sibling test, an `afterEach` hook, a nested `describe`). A callback's own
/// local declarations stay local.
/// `sites` receives the source offset of every write recorded here, so the
/// caller can tell them from writes this walk does not see.
fn collect_scope_bindings(
    stmt: &Statement<'_>,
    source: &str,
    out: &mut Vec<ScopeEntry>,
    sites: &mut Vec<usize>,
) {
    match stmt {
        Statement::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                if let BindingPattern::BindingIdentifier(identifier) = &declarator.id {
                    let value = match &declarator.init {
                        None => ScopeValue::Declared,
                        Some(init) => constructed_value(init),
                    };
                    sites.push(identifier.span.start as usize);
                    out.push((identifier.name.to_string(), value, Phase::Declaration));
                } else {
                    for identifier in declarator.id.get_binding_identifiers() {
                        out.push((
                            identifier.name.to_string(),
                            ScopeValue::Other,
                            Phase::Declaration,
                        ));
                    }
                }
            }
        }
        Statement::FunctionDeclaration(function) => {
            if let Some(identifier) = &function.id {
                out.push((
                    identifier.name.to_string(),
                    ScopeValue::Other,
                    Phase::Declaration,
                ));
            }
        }
        Statement::ClassDeclaration(class) => {
            if let Some(identifier) = &class.id {
                out.push((
                    identifier.name.to_string(),
                    ScopeValue::Other,
                    Phase::Declaration,
                ));
            }
        }
        Statement::ExpressionStatement(expr_stmt) => {
            let Expression::CallExpression(call) = &expr_stmt.expression else {
                return;
            };
            let phase = match &call.callee {
                Expression::Identifier(callee) if callee.name == "beforeAll" => Phase::BeforeAll,
                Expression::Identifier(callee) if callee.name == "beforeEach" => Phase::BeforeEach,
                _ => Phase::Interleaved,
            };
            for argument in &call.arguments {
                let Some(body) = function_body_statements_from_argument(argument) else {
                    continue;
                };
                // A generator hook's body does not run when the hook is
                // called (Vitest awaits the returned generator object).
                let phase = match argument {
                    oxc_ast::ast::Argument::FunctionExpression(function) if function.generator => {
                        Phase::Interleaved
                    }
                    _ => phase,
                };
                // Writes to the callback's own parameters and declarations
                // stay in it.
                let mut locals: Vec<ScopeEntry> = argument_parameter_names(argument)
                    .into_iter()
                    .map(|name| (name, ScopeValue::Other, Phase::Declaration))
                    .collect();
                for inner in body {
                    collect_scope_bindings(inner, source, &mut locals, &mut Vec::new());
                }
                locals.retain(|(_, _, phase)| *phase == Phase::Declaration);
                let mut writes = Vec::new();
                let mut returned = false;
                for inner in body {
                    // Only a runner hook's writes are recorded; any other
                    // callback's writes stay unrecorded, so they withhold.
                    let mut callback_sites = Vec::new();
                    let start = writes.len();
                    collect_hook_assignment(inner, source, &mut writes, &mut callback_sites);
                    if phase != Phase::Interleaved {
                        sites.extend(callback_sites);
                    }
                    // A write after a possible early `return` may not run.
                    if returned {
                        writes[start..]
                            .iter_mut()
                            .for_each(|write| write.1 = ScopeValue::Other);
                    }
                    returned |= may_return(inner, source);
                }
                let writes: Vec<ScopeEntry> = writes
                    .into_iter()
                    .filter(|(name, _)| !locals.iter().any(|(local, _, _)| local == name))
                    .map(|(name, value)| match phase {
                        Phase::Interleaved => (name, ScopeValue::Other, phase),
                        _ => (name, value, phase),
                    })
                    .collect();
                // Two hooks of one kind writing the same name may run in
                // parallel (Vitest `sequence.hooks: 'parallel'`): neither
                // write is known to be last.
                let raced: Vec<String> = writes
                    .iter()
                    .filter(|(name, _, _)| {
                        phase != Phase::Interleaved
                            && out
                                .iter()
                                .any(|(bound, _, earlier)| bound == name && *earlier == phase)
                    })
                    .map(|(name, _, _)| name.clone())
                    .collect();
                out.extend(writes);
                out.extend(
                    raced
                        .into_iter()
                        .map(|name| (name, ScopeValue::Other, phase)),
                );
            }
        }
        _ => {}
    }
}

/// A hook statement `name = <expr>;` binds `name`. Any other statement that
/// may assign a name (a nested block, a loop, a compound or destructuring
/// assignment) marks every name it assigns as ambiguous, read from its text
/// because the adapter has no nested-statement walk.
fn collect_hook_assignment(
    stmt: &Statement<'_>,
    source: &str,
    out: &mut Vec<(String, ScopeValue)>,
    sites: &mut Vec<usize>,
) {
    match stmt {
        // A hook's own declarations are local to the hook, but an
        // initializer may still write an outer name (`const r = (cart = x)`
        // or a closure that assigns it).
        Statement::VariableDeclaration(declaration) => {
            for init in declaration
                .declarations
                .iter()
                .filter_map(|declarator| declarator.init.as_ref())
            {
                let span = init.span();
                let text = source
                    .get(span.start as usize..span.end as usize)
                    .unwrap_or_default();
                for name in assigned_identifier_names(text) {
                    out.push((name, ScopeValue::Other));
                }
            }
            return;
        }
        Statement::ExpressionStatement(expr_stmt) => {
            if let Expression::AssignmentExpression(assignment) =
                expr_stmt.expression.without_parentheses()
                && assignment.operator == oxc_ast::ast::AssignmentOperator::Assign
                && let oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(target) =
                    &assignment.left
            {
                sites.push(target.span.start as usize);
                out.push((
                    target.name.to_string(),
                    constructed_value(&assignment.right),
                ));
                return;
            }
        }
        _ => {}
    }
    let span = stmt.span();
    let text = source
        .get(span.start as usize..span.end as usize)
        .unwrap_or_default();
    for name in assigned_identifier_names(text) {
        out.push((name, ScopeValue::Other));
    }
}

/// `true` when `stmt` may leave the enclosing callback early: a `return`
/// statement, or control flow (`if`, `try`, a block or loop) containing one.
/// A `return` inside an expression or declaration belongs to a nested
/// closure and does not leave the callback.
fn may_return(stmt: &Statement<'_>, source: &str) -> bool {
    if matches!(
        stmt,
        Statement::ExpressionStatement(_)
            | Statement::VariableDeclaration(_)
            | Statement::FunctionDeclaration(_)
            | Statement::ClassDeclaration(_)
    ) {
        return false;
    }
    let span = stmt.span();
    let text = source
        .get(span.start as usize..span.end as usize)
        .unwrap_or_default();
    identifier_occurrences(text, "return").next().is_some()
}

fn constructed_value(expression: &Expression<'_>) -> ScopeValue {
    let Expression::NewExpression(new_expression) = expression.without_parentheses() else {
        return ScopeValue::Other;
    };
    match &new_expression.callee {
        Expression::Identifier(identifier) => ScopeValue::Constructed(identifier.name.to_string()),
        Expression::StaticMemberExpression(member) => match &member.object {
            Expression::Identifier(object) => {
                ScopeValue::Constructed(format!("{}.{}", object.name, member.property.name))
            }
            _ => ScopeValue::Other,
        },
        _ => ScopeValue::Other,
    }
}

/// Withhold every scope receiver the file could rebind in a way the syntax
/// walk did not record. Outside comments and describe/test names, each
/// occurrence of a credited name must be a recorded declaration or hook
/// write, a member read (`cart.total()`, not `cart.total = ...`),
/// `expect(cart)` or `typeof cart`; anything else (a parameter, a cast
/// target, another declaration, an unrecorded write, text in a string an
/// `eval` could run) may rebind it. The constructor must not be declared or
/// written anywhere in the file either (`class Cart {}` or `function Cart()`
/// in a hook, `const { Cart } = ...`, `shop.Cart = ...`). A file with `eval`
/// or an escaped identifier (`\u0063art`) withholds every receiver.
fn withhold_rebound_scope_receivers(
    source: &str,
    masked: &[std::ops::Range<usize>],
    sites: &[usize],
    tests: &mut [TypeScriptTest],
) {
    let code = blank_ranges(source, masked);
    let opaque = identifier_occurrences(&code, "eval").next().is_some() || code.contains("\\u");
    let rebound: Vec<String> = identifier_writes(&code)
        .into_iter()
        .map(|(name, _)| name)
        .chain(declared_names(&code))
        .collect();
    for test in tests {
        for binding in &mut test.scope_bindings {
            let Some(constructor) = &binding.constructed_by else {
                continue;
            };
            let root = constructor.split('.').next().unwrap_or(constructor);
            // `Cart.prototype` anywhere (a spy or method assignment in a
            // hook) may replace the method the test calls.
            let constructor_rebound = rebound.iter().any(|name| name == root)
                || identifier_occurrences(&code, root).any(|at| {
                    code[at + root.len()..]
                        .trim_start()
                        .strip_prefix('.')
                        .is_some_and(|rest| rest.trim_start().starts_with("prototype"))
                })
                || (constructor.contains('.') && member_assigned(&code, constructor));
            if opaque
                || constructor_rebound
                || identifier_occurrences(&code, &binding.name)
                    .any(|at| !sites.contains(&at) && !is_plain_read(&code, at, &binding.name))
            {
                binding.constructed_by = None;
            }
        }
    }
}

/// Byte offsets of `identifier` as a whole word, not a member name.
fn identifier_occurrences<'a>(
    code: &'a str,
    identifier: &'a str,
) -> impl Iterator<Item = usize> + 'a {
    code.match_indices(identifier).filter_map(move |(at, _)| {
        let before = code[..at].chars().next_back();
        let after = code[at + identifier.len()..].chars().next();
        let word = |ch: Option<char>| {
            ch.is_some_and(|ch| ch == '_' || ch == '$' || ch.is_ascii_alphanumeric())
        };
        let member =
            code[..at].trim_end().ends_with('.') && !code[..at].trim_end().ends_with("...");
        (!word(before) && !word(after) && !member).then_some(at)
    })
}

/// `cart.x` / `cart?.x` that is not itself assigned, `expect(cart)` or
/// `typeof cart`: reads that neither rebind the name nor replace a method.
fn is_plain_read(code: &str, at: usize, identifier: &str) -> bool {
    let before = code[..at].trim_end();
    let after = code[at + identifier.len()..].trim_start();
    let member = after.strip_prefix("?.").or_else(|| {
        after
            .strip_prefix('.')
            .filter(|rest| !rest.starts_with(".."))
    });
    if let Some(mut member) = member {
        // Walk the whole chain: `cart.constructor.prototype.total = ...`
        // replaces a method as surely as `cart.total = ...`.
        loop {
            let end = member
                .find(|ch: char| !(ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()))
                .unwrap_or(member.len());
            let rest = member[end..].trim_start();
            match rest.strip_prefix("?.").or_else(|| rest.strip_prefix('.')) {
                Some(next) if !next.starts_with('.') => member = next.trim_start(),
                _ => return !assigns_next(rest),
            }
        }
    }
    (before.ends_with("expect(") && after.starts_with(')')) || before.ends_with("typeof")
}

/// `true` when `rest` starts with an assignment operator (`=`, `+=`, `??=`),
/// not a comparison or arrow.
fn assigns_next(rest: &str) -> bool {
    let rest = rest.trim_start();
    let operator_end = rest
        .find(|ch: char| {
            !matches!(
                ch,
                '+' | '-' | '*' | '/' | '%' | '&' | '|' | '^' | '?' | '<' | '>'
            )
        })
        .unwrap_or(rest.len());
    let after = &rest[operator_end..];
    after.starts_with('=')
        && !after[1..].starts_with(['=', '>'])
        && !matches!(&rest[..operator_end], "<" | ">" | "!")
}

/// `true` when `ns.Class` (as written) is assigned anywhere in `code`.
fn member_assigned(code: &str, path: &str) -> bool {
    code.match_indices(path)
        .any(|(at, _)| assigns_next(&code[at + path.len()..]))
}

/// Names declared with `class Name` or `function Name`.
fn declared_names(code: &str) -> Vec<String> {
    ["class", "function"]
        .into_iter()
        .flat_map(|keyword| {
            identifier_occurrences(code, keyword).filter_map(move |at| {
                let rest = code[at + keyword.len()..].trim_start();
                let end = rest
                    .find(|ch: char| !(ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()))
                    .unwrap_or(rest.len());
                (end > 0).then(|| rest[..end].to_string())
            })
        })
        .collect()
}

/// `source` with the ASCII bytes of `ranges` replaced by spaces, byte
/// offsets unchanged.
fn blank_ranges(source: &str, ranges: &[std::ops::Range<usize>]) -> String {
    let mut bytes = source.as_bytes().to_vec();
    for range in ranges {
        for byte in bytes.get_mut(range.clone()).into_iter().flatten() {
            if byte.is_ascii() && *byte != b'\n' {
                *byte = b' ';
            }
        }
    }
    // Only ASCII bytes were replaced, so the result is still valid UTF-8.
    String::from_utf8(bytes).unwrap_or_default()
}

/// Identifiers written as an assignment target anywhere in `text`, each
/// named once.
fn assigned_identifier_names(text: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for (name, _) in identifier_writes(text) {
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// `true` when `text` may write `identifier`; see [`identifier_writes`].
pub(crate) fn identifier_written_in(text: &str, identifier: &str) -> bool {
    identifier_writes(text)
        .iter()
        .any(|(name, _)| name == identifier)
}

/// Every identifier `text` may write, with its byte offset: an assignment
/// target (`name =`, `name +=`, `name ??=`, not `==` or `=>`), `++`/`--`, a
/// `for (name of ...)` target, and any identifier inside a bracketed group
/// that is itself assigned (`[name] = ...`, `({ name } = ...)`). Member
/// targets (`this.name =`) are not writes. Comments and strings are not
/// skipped: a false entry only withholds a relation.
fn identifier_writes(text: &str) -> Vec<(String, usize)> {
    let bytes = text.as_bytes();
    let destructured = destructuring_ranges(text);
    let mut writes = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let ch = bytes[index] as char;
        if !(ch == '_' || ch == '$' || ch.is_ascii_alphabetic()) {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && {
            let ch = bytes[index] as char;
            ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
        } {
            index += 1;
        }
        let before = text[..start].trim_end();
        if before.ends_with('.') && !before.ends_with("...") {
            continue;
        }
        let rest = text[index..].trim_start();
        let operator_end = rest
            .find(|ch: char| {
                !matches!(
                    ch,
                    '+' | '-' | '*' | '/' | '%' | '&' | '|' | '^' | '?' | '<' | '>'
                )
            })
            .unwrap_or(rest.len());
        let after_operator = &rest[operator_end..];
        let assigns = after_operator.starts_with('=')
            && !after_operator[1..].starts_with(['=', '>'])
            && !matches!(&rest[..operator_end], "<" | ">" | "!");
        let steps = rest.starts_with("++")
            || rest.starts_with("--")
            || before.ends_with("++")
            || before.ends_with("--");
        let loop_target = ["let", "const", "var"]
            .iter()
            .fold(before, |prefix, keyword| {
                prefix.strip_suffix(keyword).unwrap_or(prefix).trim_end()
            })
            .strip_suffix('(')
            .is_some_and(|prefix| {
                let prefix = prefix.trim_end();
                prefix.ends_with("for") || prefix.ends_with("await")
            });
        let in_target = destructured.iter().any(|range| range.contains(&start));
        if assigns || steps || loop_target || in_target {
            writes.push((text[start..index].to_string(), start));
        }
    }
    writes
}

/// Byte ranges of `[...]` and `{...}` groups directly followed by an
/// assignment `=`: destructuring targets.
fn destructuring_ranges(text: &str) -> Vec<std::ops::Range<usize>> {
    let bytes = text.as_bytes();
    let mut ranges = Vec::new();
    for (close, &byte) in bytes.iter().enumerate() {
        let open_byte = match byte {
            b']' => b'[',
            b'}' => b'{',
            _ => continue,
        };
        let rest = text[close + 1..].trim_start();
        if !rest.starts_with('=') || rest[1..].starts_with(['=', '>']) {
            continue;
        }
        let mut depth = 0usize;
        for open in (0..close).rev() {
            if bytes[open] == byte {
                depth += 1;
            } else if bytes[open] == open_byte {
                if depth == 0 {
                    ranges.push(open..close);
                    break;
                }
                depth -= 1;
            }
        }
    }
    ranges
}

/// Parameter names of the callback at `index` in a statement's call
/// (`describe.each(...)('x', (row) => ...)`, `it('x', ({ fixture }) => ...)`).
fn statement_callback_parameter_names(stmt: &Statement<'_>, index: usize) -> Vec<String> {
    let Statement::ExpressionStatement(expr_stmt) = stmt else {
        return Vec::new();
    };
    let Expression::CallExpression(call) = &expr_stmt.expression else {
        return Vec::new();
    };
    call.arguments
        .get(index)
        .map(argument_parameter_names)
        .unwrap_or_default()
}

/// The span of a call statement's first argument when it is a string literal.
/// The `'../src/cart'` of an import or re-export declaration.
fn module_specifier_span(stmt: &Statement<'_>) -> Option<oxc_span::Span> {
    match stmt {
        Statement::ImportDeclaration(import) => Some(import.source.span),
        Statement::ExportNamedDeclaration(export) => {
            export.source.as_ref().map(|source| source.span)
        }
        Statement::ExportAllDeclaration(export) => Some(export.source.span),
        _ => None,
    }
}

fn name_literal_span(stmt: &Statement<'_>) -> Option<std::ops::Range<usize>> {
    let Statement::ExpressionStatement(expr_stmt) = stmt else {
        return None;
    };
    let Expression::CallExpression(call) = &expr_stmt.expression else {
        return None;
    };
    match call.arguments.first()? {
        oxc_ast::ast::Argument::StringLiteral(literal) => {
            Some(literal.span.start as usize..literal.span.end as usize)
        }
        _ => None,
    }
}

/// Names a callback argument binds as parameters; empty for anything else.
fn argument_parameter_names(argument: &oxc_ast::ast::Argument<'_>) -> Vec<String> {
    let params = match argument {
        oxc_ast::ast::Argument::ArrowFunctionExpression(arrow) => &arrow.params,
        oxc_ast::ast::Argument::FunctionExpression(function) => &function.params,
        _ => return Vec::new(),
    };
    params
        .items
        .iter()
        .flat_map(|param| param.pattern.get_binding_identifiers())
        .map(|identifier| identifier.name.to_string())
        .collect()
}

pub(crate) fn describe_body_from_statement<'a>(
    stmt: &'a Statement<'a>,
    source: &SourceText<'_>,
) -> Option<(String, &'a oxc_allocator::Vec<'a, Statement<'a>>)> {
    let Statement::ExpressionStatement(expr_stmt) = stmt else {
        return None;
    };
    let Expression::CallExpression(call) = &expr_stmt.expression else {
        return None;
    };
    if !call_callee_is_active_declaration(call, TestDeclarationRoot::Describe)
        && !call_callee_is_active_each_declaration(call, TestDeclarationRoot::Describe)
    {
        return None;
    }
    let name = title_argument(call.arguments.first()?, source, call.span.start as usize)?;
    let body = function_body_statements_from_argument(call.arguments.get(1)?)?;
    Some((name, body))
}

pub(crate) fn test_from_statement(
    stmt: &Statement<'_>,
    file: &Path,
    source: &SourceText<'_>,
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
        line: source.line_for_offset(call.span.start as usize),
        body_text: source[call.span.start as usize..call.span.end as usize].to_string(),
        assertions,
        // Populated by `extract_tests` (the only public extractor) once
        // per file before the test is returned to the caller.
        mocks_in_file: Vec::new(),
        imports_in_file: Vec::new(),
        scope_bindings: Vec::new(),
    })
}

pub(crate) fn test_name_and_assertions_from_call(
    call: &oxc_ast::ast::CallExpression<'_>,
    source: &SourceText<'_>,
) -> Option<(String, Vec<TypeScriptAssertion>)> {
    if !call_callee_is_active_declaration(call, TestDeclarationRoot::Test)
        && !call_callee_is_active_each_declaration(call, TestDeclarationRoot::Test)
    {
        return None;
    }

    let name = title_argument(call.arguments.first()?, source, call.span.start as usize)?;
    let callback = call.arguments.get(1)?;
    let receiver = test_callback_receiver_name(callback);
    let assertions = function_body_statements_from_argument(callback)
        .map(|statements| {
            collect_expect_assertions_in_statements(statements, source, receiver.as_deref())
        })
        .unwrap_or_default();
    Some((name, assertions))
}

/// Extract the name bound to the test callback's first parameter.
///
/// AVA / node:test / tape pass an execution context (conventionally `t`) as the
/// first argument of the test callback, and assertions are made on it
/// (`t.is(...)`, `t.deepEqual(...)`). Jest/Vitest callbacks take no such
/// receiver, so this returns `None` for a zero-parameter callback and the AVA
/// assertion matcher is never attempted (fail-closed: no receiver, no AVA
/// assertions credited).
fn test_callback_receiver_name(arg: &oxc_ast::ast::Argument<'_>) -> Option<String> {
    let params = match arg {
        oxc_ast::ast::Argument::ArrowFunctionExpression(arrow) => &arrow.params,
        oxc_ast::ast::Argument::FunctionExpression(func) => &func.params,
        _ => return None,
    };
    let first = params.items.first()?;
    super::owners::binding_identifier_name(&first.pattern).map(|name| name.to_string())
}

fn call_callee_is_active_declaration(
    call: &oxc_ast::ast::CallExpression<'_>,
    root: TestDeclarationRoot,
) -> bool {
    expression_is_active_declaration(&call.callee, root)
}

fn call_callee_is_active_each_declaration(
    call: &oxc_ast::ast::CallExpression<'_>,
    root: TestDeclarationRoot,
) -> bool {
    let Expression::CallExpression(each_call) = &call.callee else {
        return false;
    };
    let Expression::StaticMemberExpression(member) = &each_call.callee else {
        return false;
    };
    member.property.name.as_str() == "each"
        && expression_is_active_declaration(&member.object, root)
}

fn expression_is_active_declaration(
    expression: &Expression<'_>,
    root: TestDeclarationRoot,
) -> bool {
    match expression {
        Expression::Identifier(ident) => root.matches_identifier(ident.name.as_str()),
        Expression::StaticMemberExpression(member) => {
            is_active_declaration_modifier(member.property.name.as_str())
                && expression_is_active_declaration(&member.object, root)
        }
        _ => false,
    }
}

fn is_active_declaration_modifier(name: &str) -> bool {
    matches!(name, "only" | "concurrent" | "sequential")
}

/// The title of a `describe`/`test`/`it` registration. A string literal or a
/// template without substitutions keeps its text. Any other expression (a
/// template with substitutions, a concatenation, an identifier) is a
/// computed title: it is named by a stable placeholder and the line of the
/// registration, so the name is deterministic and never guessed from the
/// runtime value. A spread argument is not a title.
fn title_argument(
    arg: &oxc_ast::ast::Argument<'_>,
    source: &SourceText<'_>,
    registration_start: usize,
) -> Option<String> {
    match arg {
        oxc_ast::ast::Argument::StringLiteral(literal) => Some(literal.value.to_string()),
        oxc_ast::ast::Argument::SpreadElement(_) => None,
        oxc_ast::ast::Argument::TemplateLiteral(template) if template.single_quasi().is_some() => {
            template.single_quasi().map(|quasi| quasi.to_string())
        }
        _ => Some(computed_title(source.line_for_offset(registration_start))),
    }
}

/// Placeholder name of a registration whose title is computed at runtime.
fn computed_title(line: usize) -> String {
    format!("<computed title, line {line}>")
}

/// The body of a loop that registers tests once per iteration, with the
/// names its header binds: `for (const t of tests) { ... }`,
/// `for (const key in table) { ... }`, `for (let i = 0; i < 3; i++) { ... }`
/// and `tests.forEach((t) => { ... })`. A loop is walked only when it is
/// known to run at least once: its iterable is a non-empty array or object
/// literal, `Object.entries`/`keys`/`values` of one, or a `const` in scope
/// bound to one; a counted `for` needs literal bounds. Tests registered by
/// a loop that may run zero times would be credited without existing, so
/// every other loop is left unwalked (and stays disclosed as partial), as is
/// a `for...of`/`for...in` whose target is not a declaration.
fn loop_body_from_statement<'a>(
    stmt: &'a Statement<'a>,
    scope: &TestScope,
) -> Option<(Vec<String>, &'a [Statement<'a>])> {
    let (bindings, body) = match stmt {
        Statement::ForOfStatement(for_of) => {
            if !iterates_nonempty(&for_of.right, scope) {
                return None;
            }
            (for_left_bindings(&for_of.left)?, &for_of.body)
        }
        Statement::ForInStatement(for_in) => {
            if !enumerates_nonempty(&for_in.right, scope) {
                return None;
            }
            (for_left_bindings(&for_in.left)?, &for_in.body)
        }
        Statement::ForStatement(for_stmt) => {
            let Some(oxc_ast::ast::ForStatementInit::VariableDeclaration(declaration)) =
                &for_stmt.init
            else {
                return None;
            };
            if !counted_loop_runs(declaration, for_stmt.test.as_ref()) {
                return None;
            }
            (declaration_binding_names(declaration), &for_stmt.body)
        }
        Statement::ExpressionStatement(expr_stmt) => {
            let Expression::CallExpression(call) = &expr_stmt.expression else {
                return None;
            };
            let Expression::StaticMemberExpression(member) = &call.callee else {
                return None;
            };
            if member.property.name.as_str() != "forEach"
                || !iterates_nonempty(&member.object, scope)
            {
                return None;
            }
            let callback = call.arguments.first()?;
            let body = function_body_statements_from_argument(callback)?;
            let bindings = argument_parameter_names(callback);
            if binds_registration_api(&bindings) {
                return None;
            }
            return Some((bindings, body.as_slice()));
        }
        _ => return None,
    };
    if binds_registration_api(&bindings) {
        return None;
    }
    let body = match body {
        Statement::BlockStatement(block) => block.body.as_slice(),
        single => std::slice::from_ref(single),
    };
    Some((bindings, body))
}

/// A loop variable or callback parameter named `test`, `it` or `describe`
/// shadows the runner API inside the body, so a `test(...)` there is not a
/// registration; such a loop is not walked.
fn binds_registration_api(bindings: &[String]) -> bool {
    bindings.iter().any(|name| {
        TestDeclarationRoot::Test.matches_identifier(name)
            || TestDeclarationRoot::Describe.matches_identifier(name)
    })
}

fn for_left_bindings(left: &oxc_ast::ast::ForStatementLeft<'_>) -> Option<Vec<String>> {
    match left {
        oxc_ast::ast::ForStatementLeft::VariableDeclaration(declaration) => {
            Some(declaration_binding_names(declaration))
        }
        _ => None,
    }
}

fn declaration_binding_names(declaration: &oxc_ast::ast::VariableDeclaration<'_>) -> Vec<String> {
    declaration
        .declarations
        .iter()
        .flat_map(|declarator| declarator.id.get_binding_identifiers())
        .map(|identifier| identifier.name.to_string())
        .collect()
}

/// A literal a `const` is bound to, when it has at least one element or
/// property that is not a spread.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IterableLiteral {
    Array,
    Object,
}

/// Record `const name = <non-empty array or object literal>` declarations.
fn collect_const_iterables(stmt: &Statement<'_>, out: &mut Vec<(String, IterableLiteral)>) {
    let Statement::VariableDeclaration(declaration) = stmt else {
        return;
    };
    if declaration.kind != oxc_ast::ast::VariableDeclarationKind::Const {
        return;
    }
    for declarator in &declaration.declarations {
        if let (BindingPattern::BindingIdentifier(identifier), Some(init)) =
            (&declarator.id, &declarator.init)
            && let Some(kind) = nonempty_literal(init)
        {
            out.push((identifier.name.to_string(), kind));
        }
    }
}

/// Strip parentheses and TypeScript-only wrappers (`as`, `satisfies`, `!`,
/// `<T>x`), which do not change the runtime value.
fn runtime_expression<'a, 'b>(expression: &'b Expression<'a>) -> &'b Expression<'a> {
    match expression {
        Expression::ParenthesizedExpression(inner) => runtime_expression(&inner.expression),
        Expression::TSAsExpression(inner) => runtime_expression(&inner.expression),
        Expression::TSSatisfiesExpression(inner) => runtime_expression(&inner.expression),
        Expression::TSNonNullExpression(inner) => runtime_expression(&inner.expression),
        Expression::TSTypeAssertion(inner) => runtime_expression(&inner.expression),
        other => other,
    }
}

fn nonempty_literal(expression: &Expression<'_>) -> Option<IterableLiteral> {
    match runtime_expression(expression) {
        Expression::ArrayExpression(array)
            if array.elements.iter().any(|element| {
                !matches!(
                    element,
                    oxc_ast::ast::ArrayExpressionElement::SpreadElement(_)
                )
            }) =>
        {
            Some(IterableLiteral::Array)
        }
        Expression::ObjectExpression(object)
            if object.properties.iter().any(|property| {
                matches!(
                    property,
                    oxc_ast::ast::ObjectPropertyKind::ObjectProperty(_)
                )
            }) =>
        {
            Some(IterableLiteral::Object)
        }
        _ => None,
    }
}

/// The literal `expression` is known to evaluate to: a literal itself, or an
/// identifier whose innermost binding in scope is a single `const`
/// declaration of one (a hook write, a parameter, a loop variable or any
/// second binding of the name at that level makes it unknown).
fn known_literal(expression: &Expression<'_>, scope: &TestScope) -> Option<IterableLiteral> {
    let expression = runtime_expression(expression);
    if let Some(kind) = nonempty_literal(expression) {
        return Some(kind);
    }
    let Expression::Identifier(identifier) = expression else {
        return None;
    };
    let name = identifier.name.as_str();
    let depth = scope
        .levels
        .iter()
        .rposition(|level| level.iter().any(|(bound, _, _)| bound == name))?;
    let writes = scope
        .levels
        .get(depth)?
        .iter()
        .filter(|(bound, _, _)| bound == name);
    if writes.count() != 1 {
        return None;
    }
    scope
        .iterables
        .get(depth)?
        .iter()
        .find(|(bound, _)| bound == name)
        .map(|(_, kind)| *kind)
}

/// `for...of` / `.forEach` over a value known to yield at least one item: a
/// non-empty array, or `Object.entries`/`keys`/`values` of a non-empty
/// object or array.
fn iterates_nonempty(expression: &Expression<'_>, scope: &TestScope) -> bool {
    if known_literal(expression, scope) == Some(IterableLiteral::Array) {
        return true;
    }
    let Expression::CallExpression(call) = runtime_expression(expression) else {
        return false;
    };
    let Expression::StaticMemberExpression(member) = &call.callee else {
        return false;
    };
    let is_object_enumeration = matches!(&member.object, Expression::Identifier(object) if object.name == "Object")
        && matches!(member.property.name.as_str(), "entries" | "keys" | "values");
    is_object_enumeration
        && call.arguments.len() == 1
        && call
            .arguments
            .first()
            .and_then(|argument| argument.as_expression())
            .is_some_and(|argument| known_literal(argument, scope).is_some())
}

/// `for...in` over a value known to have at least one enumerable key.
fn enumerates_nonempty(expression: &Expression<'_>, scope: &TestScope) -> bool {
    known_literal(expression, scope).is_some()
}

/// `for (let i = <a>; i < <b> | i <= <b>; ...)` with numeric literal bounds
/// that admit the first iteration.
fn counted_loop_runs(
    declaration: &oxc_ast::ast::VariableDeclaration<'_>,
    test: Option<&Expression<'_>>,
) -> bool {
    let [declarator] = declaration.declarations.as_slice() else {
        return false;
    };
    let (BindingPattern::BindingIdentifier(counter), Some(init)) =
        (&declarator.id, &declarator.init)
    else {
        return false;
    };
    let Expression::NumericLiteral(start) = runtime_expression(init) else {
        return false;
    };
    let Some(Expression::BinaryExpression(comparison)) = test.map(runtime_expression) else {
        return false;
    };
    let (Expression::Identifier(left), Expression::NumericLiteral(bound)) = (
        runtime_expression(&comparison.left),
        runtime_expression(&comparison.right),
    ) else {
        return false;
    };
    if left.name != counter.name {
        return false;
    }
    match comparison.operator {
        oxc_ast::ast::BinaryOperator::LessThan => start.value < bound.value,
        oxc_ast::ast::BinaryOperator::LessEqualThan => start.value <= bound.value,
        _ => false,
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

/// Detect test registrations that the syntax-first extractor silently drops
/// from a recognized test file that parses cleanly.
///
/// Real producer for the `typescript_test_extraction_partial` named limitation
/// (classification-neutral additive disclosure): the test index still contains
/// exactly what `extract_tests` extracted; this function only reports that the
/// index is PARTIAL, so consumers know a confident `no_static_path` can be a
/// false negative for owners whose only tests use unsupported shapes.
///
/// Detected shapes (bounded preview slice — extracting these shapes is a
/// separate backlog item; this lane only discloses them):
///
/// - `` test.each`table`('name', fn) `` / `` it.each`table`('name', fn) `` —
///   tagged-template `.each` (the tagged template sits in callee position, so
///   the extractor's identifier/member callee check never recognizes it).
/// - `it(...)` / `test(...)` calls nested inside a body the extractor does not
///   walk: an `if`/`while`/`try` block, a helper function, or a callback other
///   than `describe(...)` and `.forEach(...)`, or a loop not known to run at
///   least once. `for`, `for...of` and `for...in` bodies and `.forEach`
///   callbacks over a known non-empty value are walked (with computed titles
///   named by placeholder), so their registrations are extracted.
///   Such a call reports `template-literal title` when its title is a
///   template literal.
///
/// Returns `None` for a fully extracted file (the negative control contract):
/// every test-shaped call at a position the extractor visits carries a string
/// literal title and therefore already appears in `extracted`.
pub(crate) fn detect_partial_test_extraction(
    file: &Path,
    source: &str,
    extracted: &[TypeScriptTest],
) -> Option<TypeScriptTestExtractionGap> {
    // Span starts are pure string search. Compute them before the worker:
    // the parse closure is `'static` and cannot borrow `extracted`.
    let extracted_starts = extracted_span_starts(source, extracted);
    let Ok(gap) = parse_on_worker(file, source, move |file, source, allocator| {
        let ret = Parser::new(allocator, source, source_type_for(file)).parse();
        if !ret.errors.is_empty() {
            // Parse-error disclosure owns this case; do not double-report.
            return None;
        }
        let source = SourceText::new(source);
        let mut finder = UnextractedTestFinder {
            source: &source,
            extracted_starts: &extracted_starts,
            gap: None,
        };
        finder.visit_statements(&ret.program.body);
        finder.gap.map(
            |(sample_line, shape, snippet)| TypeScriptTestExtractionGap {
                file: file.to_path_buf(),
                sample_line,
                shape,
                snippet,
            },
        )
    }) else {
        return None;
    };
    gap
}

/// Recover the byte-offset span start for every extracted test from its
/// recorded `body_text` (the exact source slice of the call), so the
/// finder can tell extracted registrations apart from dropped ones without
/// threading span state through the public extractor signature. Extraction
/// visits registrations in source order, so a forward watermark disambiguates
/// identical bodies on the same line.
fn extracted_span_starts(source: &str, extracted: &[TypeScriptTest]) -> Vec<usize> {
    let mut watermark = 0usize;
    let mut starts = Vec::new();
    for test in extracted {
        if let Some(pos) = source
            .get(watermark..)
            .and_then(|rest| rest.find(test.body_text.as_str()))
        {
            let start = watermark + pos;
            starts.push(start);
            // The next extracted test starts strictly after this one.
            watermark = start + 1;
        }
    }
    starts
}

/// Bounded AST walk that flags the first test-shaped registration the
/// extractor did not index. Intentionally a separate, shallower walk than the
/// extractor: it only needs enough recursion to reach realistic dropped shapes
/// (statement containers, call arguments, function bodies). Array-element and
/// object-property subtrees are out of scope for this disclosure slice.
struct UnextractedTestFinder<'a> {
    source: &'a SourceText<'a>,
    extracted_starts: &'a [usize],
    gap: Option<(usize, &'static str, String)>,
}

impl UnextractedTestFinder<'_> {
    fn visit_statements(&mut self, statements: &[Statement<'_>]) {
        for stmt in statements {
            if self.gap.is_some() {
                return;
            }
            self.visit_statement(stmt);
        }
    }

    fn visit_statement(&mut self, stmt: &Statement<'_>) {
        match stmt {
            Statement::BlockStatement(block) => self.visit_statements(&block.body),
            Statement::ExpressionStatement(expr_stmt) => {
                self.visit_expression(&expr_stmt.expression, true);
            }
            Statement::ReturnStatement(return_stmt) => {
                if let Some(argument) = &return_stmt.argument {
                    self.visit_expression(argument, false);
                }
            }
            Statement::ThrowStatement(throw_stmt) => {
                self.visit_expression(&throw_stmt.argument, false);
            }
            Statement::IfStatement(if_stmt) => {
                self.visit_statement(&if_stmt.consequent);
                if let Some(alternate) = &if_stmt.alternate {
                    self.visit_statement(alternate);
                }
            }
            Statement::DoWhileStatement(do_while) => self.visit_statement(&do_while.body),
            Statement::WhileStatement(while_stmt) => self.visit_statement(&while_stmt.body),
            Statement::ForStatement(for_stmt) => self.visit_statement(&for_stmt.body),
            Statement::ForInStatement(for_in) => self.visit_statement(&for_in.body),
            Statement::ForOfStatement(for_of) => self.visit_statement(&for_of.body),
            Statement::LabeledStatement(labeled) => self.visit_statement(&labeled.body),
            Statement::SwitchStatement(switch_stmt) => {
                for case in &switch_stmt.cases {
                    self.visit_statements(&case.consequent);
                    if self.gap.is_some() {
                        return;
                    }
                }
            }
            Statement::TryStatement(try_stmt) => {
                self.visit_statements(&try_stmt.block.body);
                if self.gap.is_none()
                    && let Some(handler) = &try_stmt.handler
                {
                    self.visit_statements(&handler.body.body);
                }
                if self.gap.is_none()
                    && let Some(finalizer) = &try_stmt.finalizer
                {
                    self.visit_statements(&finalizer.body);
                }
            }
            Statement::WithStatement(with_stmt) => self.visit_statement(&with_stmt.body),
            Statement::VariableDeclaration(decl) => {
                for declarator in &decl.declarations {
                    if let Some(init) = &declarator.init {
                        self.visit_expression(init, false);
                    }
                    if self.gap.is_some() {
                        return;
                    }
                }
            }
            Statement::FunctionDeclaration(func) => self.visit_function(func),
            Statement::ClassDeclaration(class) => self.visit_class(class),
            Statement::ExportDefaultDeclaration(export_default) => {
                match &export_default.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                        self.visit_function(func);
                    }
                    ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                        self.visit_class(class);
                    }
                    _ => {}
                }
            }
            Statement::ExportNamedDeclaration(export_named) => {
                match export_named.declaration.as_ref() {
                    Some(Declaration::FunctionDeclaration(func)) => self.visit_function(func),
                    Some(Declaration::ClassDeclaration(class)) => self.visit_class(class),
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn visit_function(&mut self, func: &Function<'_>) {
        if let Some(body) = &func.body {
            self.visit_statements(&body.statements);
        }
    }

    fn visit_class(&mut self, class: &Class<'_>) {
        for element in &class.body.body {
            match element {
                ClassElement::StaticBlock(block) => self.visit_statements(&block.body),
                ClassElement::MethodDefinition(method) => self.visit_function(&method.value),
                _ => {}
            }
            if self.gap.is_some() {
                return;
            }
        }
    }

    fn visit_expression(&mut self, expr: &Expression<'_>, statement_position: bool) {
        match expr {
            Expression::CallExpression(call) => self.visit_call(call, statement_position),
            Expression::NewExpression(new_expr) => self.visit_arguments(&new_expr.arguments),
            Expression::TaggedTemplateExpression(tagged) => {
                self.visit_tagged_template(tagged, statement_position);
            }
            Expression::ArrowFunctionExpression(arrow) => {
                self.visit_statements(&arrow.body.statements);
            }
            Expression::FunctionExpression(func) => self.visit_function(func),
            Expression::ClassExpression(class) => self.visit_class(class),
            Expression::AwaitExpression(await_expr) => {
                self.visit_expression(&await_expr.argument, false);
            }
            Expression::UnaryExpression(unary) => {
                self.visit_expression(&unary.argument, false);
            }
            // `UpdateExpression.argument` is an assignment target and
            // `ChainExpression.expression` is a `ChainElement`; neither can
            // contain a statement-position test registration, so both are
            // intentionally not recursed into.
            Expression::YieldExpression(yield_expr) => {
                if let Some(argument) = &yield_expr.argument {
                    self.visit_expression(argument, false);
                }
            }
            Expression::BinaryExpression(binary) => {
                self.visit_expression(&binary.left, false);
                self.visit_expression(&binary.right, false);
            }
            Expression::LogicalExpression(logical) => {
                self.visit_expression(&logical.left, false);
                self.visit_expression(&logical.right, false);
            }
            Expression::ConditionalExpression(conditional) => {
                self.visit_expression(&conditional.consequent, false);
                self.visit_expression(&conditional.alternate, false);
            }
            Expression::AssignmentExpression(assignment) => {
                self.visit_expression(&assignment.right, false);
            }
            Expression::SequenceExpression(sequence) => {
                for item in &sequence.expressions {
                    self.visit_expression(item, false);
                }
            }
            Expression::ParenthesizedExpression(parenthesized) => {
                self.visit_expression(&parenthesized.expression, false);
            }
            Expression::StaticMemberExpression(member) => {
                self.visit_expression(&member.object, false);
            }
            Expression::ComputedMemberExpression(member) => {
                self.visit_expression(&member.object, false);
                self.visit_expression(&member.expression, false);
            }
            Expression::PrivateFieldExpression(member) => {
                self.visit_expression(&member.object, false);
            }
            Expression::ImportExpression(import) => {
                self.visit_expression(&import.source, false);
                if let Some(options) = &import.options {
                    self.visit_expression(options, false);
                }
            }
            Expression::TemplateLiteral(template) => {
                for item in &template.expressions {
                    self.visit_expression(item, false);
                }
            }
            _ => {}
        }
    }

    /// A `test(...)` / `it(...)` call — including `.each(...)` and modifier
    /// chains — that the extractor did not index. Only checked at statement
    /// position: nested calls (assertions, helpers) are never registrations.
    fn visit_call(&mut self, call: &oxc_ast::ast::CallExpression<'_>, statement_position: bool) {
        if statement_position && self.gap.is_none() {
            // Tagged-template `.each`: `` test.each`table`('name', fn) `` — the
            // tagged template sits in CALLEE position, so the call's callee is
            // not an identifier/member the extractor recognizes at all; the
            // registration is dropped regardless of the title shape.
            if let Expression::TaggedTemplateExpression(tagged) = &call.callee
                && !self.extracted_starts.contains(&(call.span.start as usize))
                && tagged_template_tag_is_test_each(&tagged.tag)
            {
                self.gap = Some((
                    self.source.line_for_offset(call.span.start as usize),
                    "tagged-template .each",
                    snippet_for_span(
                        self.source,
                        call.span.start as usize,
                        call.span.end as usize,
                    ),
                ));
                return;
            }
            if !self.extracted_starts.contains(&(call.span.start as usize))
                && (call_callee_is_active_declaration(call, TestDeclarationRoot::Test)
                    || call_callee_is_active_each_declaration(call, TestDeclarationRoot::Test))
            {
                let shape = match call.arguments.first() {
                    Some(Argument::TemplateLiteral(_)) => "template-literal title",
                    _ => "test/it call in loop/callback/nested body",
                };
                self.gap = Some((
                    self.source.line_for_offset(call.span.start as usize),
                    shape,
                    snippet_for_span(
                        self.source,
                        call.span.start as usize,
                        call.span.end as usize,
                    ),
                ));
                return;
            }
        }
        self.visit_arguments(&call.arguments);
    }

    fn visit_arguments(&mut self, arguments: &[Argument<'_>]) {
        for argument in arguments {
            if self.gap.is_some() {
                return;
            }
            self.visit_argument(argument);
        }
    }

    /// Recurse into a call argument. `Argument` inherits every `Expression`
    /// variant but is a distinct type with no borrowed conversion, so dispatch
    /// the container shapes the walker recurses into explicitly.
    fn visit_argument(&mut self, argument: &Argument<'_>) {
        match argument {
            Argument::SpreadElement(spread) => self.visit_expression(&spread.argument, false),
            Argument::CallExpression(call) => self.visit_call(call, false),
            Argument::NewExpression(new_expr) => self.visit_arguments(&new_expr.arguments),
            Argument::TaggedTemplateExpression(tagged) => {
                self.visit_tagged_template(tagged, false);
            }
            Argument::ArrowFunctionExpression(arrow) => {
                self.visit_statements(&arrow.body.statements);
            }
            Argument::FunctionExpression(func) => self.visit_function(func),
            Argument::ClassExpression(class) => self.visit_class(class),
            Argument::AwaitExpression(await_expr) => {
                self.visit_expression(&await_expr.argument, false);
            }
            Argument::ParenthesizedExpression(parenthesized) => {
                self.visit_expression(&parenthesized.expression, false);
            }
            Argument::SequenceExpression(sequence) => {
                for item in &sequence.expressions {
                    self.visit_expression(item, false);
                }
            }
            Argument::ConditionalExpression(conditional) => {
                self.visit_expression(&conditional.consequent, false);
                self.visit_expression(&conditional.alternate, false);
            }
            Argument::LogicalExpression(logical) => {
                self.visit_expression(&logical.left, false);
                self.visit_expression(&logical.right, false);
            }
            Argument::BinaryExpression(binary) => {
                self.visit_expression(&binary.left, false);
                self.visit_expression(&binary.right, false);
            }
            Argument::StaticMemberExpression(member) => {
                self.visit_expression(&member.object, false);
            }
            Argument::ComputedMemberExpression(member) => {
                self.visit_expression(&member.object, false);
                self.visit_expression(&member.expression, false);
            }
            Argument::PrivateFieldExpression(member) => {
                self.visit_expression(&member.object, false);
            }
            Argument::TemplateLiteral(template) => {
                for item in &template.expressions {
                    self.visit_expression(item, false);
                }
            }
            _ => {}
        }
    }

    fn visit_tagged_template(
        &mut self,
        tagged: &oxc_ast::ast::TaggedTemplateExpression<'_>,
        statement_position: bool,
    ) {
        if statement_position && self.gap.is_none() && tagged_template_tag_is_test_each(&tagged.tag)
        {
            self.gap = Some((
                self.source.line_for_offset(tagged.span.start as usize),
                "tagged-template .each",
                snippet_for_span(
                    self.source,
                    tagged.span.start as usize,
                    tagged.span.end as usize,
                ),
            ));
            return;
        }
        self.visit_expression(&tagged.tag, false);
        for quasi_expr in &tagged.quasi.expressions {
            self.visit_expression(quasi_expr, false);
        }
    }
}

/// Whether a tagged template's tag is the `test.each` / `it.each` tagged-template
/// form (`` test.each`table` ``), as opposed to the call form
/// `test.each([...])(...)` handled by `call_callee_is_active_each_declaration`.
fn tagged_template_tag_is_test_each(tag: &Expression<'_>) -> bool {
    let Expression::StaticMemberExpression(member) = tag else {
        return false;
    };
    member.property.name.as_str() == "each"
        && expression_is_active_declaration(&member.object, TestDeclarationRoot::Test)
}

/// Single-line, length-bounded source snippet for limitation details.
pub(crate) fn snippet_for_span(source: &str, start: usize, end: usize) -> String {
    let snippet: String = source
        .get(start..end)
        .unwrap_or_default()
        .chars()
        .take(80)
        .collect();
    let collapsed = snippet.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() > 77 {
        format!("{}...", collapsed.chars().take(77).collect::<String>())
    } else {
        collapsed
    }
}
