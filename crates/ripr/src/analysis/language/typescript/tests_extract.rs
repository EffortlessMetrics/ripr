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
    }) else {
        return Vec::new();
    };
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
    if !call_callee_is_active_declaration(call, TestDeclarationRoot::Describe)
        && !call_callee_is_active_each_declaration(call, TestDeclarationRoot::Describe)
    {
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
    if !call_callee_is_active_declaration(call, TestDeclarationRoot::Test)
        && !call_callee_is_active_each_declaration(call, TestDeclarationRoot::Test)
    {
        return None;
    }

    let name = string_argument(call.arguments.first()?)?;
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
/// - `` it(`title ${x}`, fn) `` / `` test(`title`, fn) `` — template-literal
///   titles (`string_argument` accepts `StringLiteral` only).
/// - `` test.each`table`('name', fn) `` / `` it.each`table`('name', fn) `` —
///   tagged-template `.each` (the tagged template sits in callee position, so
///   the extractor's identifier/member callee check never recognizes it).
/// - `it(...)` / `test(...)` calls nested inside loop, callback, or other
///   non-`describe` bodies — `collect_tests_from_statements` recurses only into
///   `describe(...)` bodies.
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
        let mut finder = UnextractedTestFinder {
            source,
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
    source: &'a str,
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
                    line_for_offset(self.source, call.span.start as usize),
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
                    line_for_offset(self.source, call.span.start as usize),
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
                line_for_offset(self.source, tagged.span.start as usize),
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
