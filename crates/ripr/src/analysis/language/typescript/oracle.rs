//! Oracle analysis for the TypeScript preview adapter.

use super::*;

pub(crate) fn oracle_for_matcher(matcher: &str) -> (OracleKind, OracleStrength) {
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

/// Map an AVA / `node:test`-style assertion method (`t.is`, `t.deepEqual`, ...)
/// to an oracle kind + strength. AVA's `t.is(actual, expected)` is the structural
/// equivalent of Jest's `expect(actual).toBe(expected)` — an exact-value
/// discriminator. Negated equality (`t.not(...)`, `t.notDeepEqual(...)`, and
/// tape/node aliases) reaches the value but does not pin the exact expected
/// discriminator, so it stays relational/weak. Unknown methods return `Unknown`
/// (fail-closed): a strong oracle is only credited for an explicitly recognized
/// exact-value form.
pub(crate) fn oracle_for_ava_assertion(method: &str) -> (OracleKind, OracleStrength) {
    match method {
        // Positive equality / deep equality — discriminates an exact value.
        "is" | "equal" | "strictEqual" | "deepEqual" => {
            (OracleKind::ExactValue, OracleStrength::Strong)
        }
        // Negated equality — observes a relationship but not the exact changed
        // value. Keeping this weak prevents non-equality from becoming an
        // exact-value discriminator.
        "not" | "notEqual" | "notStrictEqual" | "notDeepEqual" => {
            (OracleKind::RelationalCheck, OracleStrength::Weak)
        }
        // Truthiness — does not pin the exact changed value. `truthy` / `falsy`
        // / `pass` / `fail` / `assert` are AVA; `ok` / `notOk` are tape.
        "true" | "false" | "truthy" | "falsy" | "pass" | "fail" | "assert" | "ok" | "notOk" => {
            (OracleKind::SmokeOnly, OracleStrength::Smoke)
        }
        // Error assertions — broad until the thrown payload is inspected.
        "throws" | "throwsAsync" | "notThrows" | "notThrowsAsync" => {
            (OracleKind::BroadError, OracleStrength::Weak)
        }
        // Partial / pattern checks.
        "regex" | "notRegex" | "like" | "notLike" => {
            (OracleKind::RelationalCheck, OracleStrength::Weak)
        }
        _ => (OracleKind::Unknown, OracleStrength::Unknown),
    }
}

pub(crate) fn weak_oracle_missing_summary(
    owner_name: &str,
    oracle_kind: &OracleKind,
    probe_family: &ProbeFamily,
    mock_payload_oracle: Option<&str>,
) -> String {
    match oracle_kind {
        OracleKind::Snapshot => format!(
            "Related test reaches `{owner_name}` with snapshot evidence; keep the snapshot as weak preview evidence and add an exact-value assertion for the changed discriminator before routing a repair packet."
        ),
        OracleKind::SmokeOnly => format!(
            "Related test reaches `{owner_name}` with a smoke-only oracle; replace or augment the truthiness check with an exact-value assertion for the changed discriminator before routing a repair packet."
        ),
        OracleKind::MockExpectation if matches!(probe_family, ProbeFamily::SideEffect) => {
            mock_payload_oracle.map_or_else(
                || format!(
                    "Related test reaches `{owner_name}` with a mock interaction oracle, but TypeScript preview does not yet establish the changed call payload; keep the item advisory until mock-shape actionability can name the callee, expected arguments, verify command, receipt command, and edit boundaries."
                ),
                |oracle| format!(
                    "Related test reaches `{owner_name}` with bounded mock payload evidence `{oracle}`; keep the item advisory until mock-shape actionability can name verify command, receipt command, evidence refs, and edit boundaries."
                ),
            )
        }
        OracleKind::BroadError => format!(
            "Related test reaches `{owner_name}` with broad error evidence; keep it weak until TypeScript preview can establish the thrown or rejected payload and emit a bounded error-path repair packet."
        ),
        _ => format!(
            "Related test reaches `{owner_name}` but the strongest extracted oracle is `{}`; upgrade by adding an exact-value (`toBe` / `toEqual` / `toStrictEqual`) assertion or a `toThrow` form with an exact payload (string, object, or class reference).",
            oracle_kind.as_str()
        ),
    }
}

pub(crate) fn weak_oracle_recommendation(
    oracle_kind: &OracleKind,
    discriminator: &str,
    mock_payload_oracle: Option<&str>,
) -> String {
    match oracle_kind {
        OracleKind::Snapshot => format!(
            "TypeScript preview advisory: add an exact-value assertion alongside the snapshot for missing discriminator `{discriminator}`; no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available."
        ),
        OracleKind::SmokeOnly => format!(
            "TypeScript preview advisory: replace or augment the smoke-only assertion with an exact-value assertion for missing discriminator `{discriminator}`; no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available."
        ),
        OracleKind::MockExpectation => mock_payload_oracle.map_or_else(
                || format!(
                    "TypeScript preview advisory: related mock interaction evidence is present, but mock payloads are not yet a safe discriminator for `{discriminator}`; no actionable repair packet is emitted until mock-shape support can name verify, receipt, evidence refs, and edit boundaries."
                ),
                |oracle| format!(
                    "TypeScript preview advisory: related mock payload evidence `{oracle}` is syntax-bounded for `{discriminator}`, but no actionable repair packet is emitted until verify, receipt, evidence refs, and edit boundaries are available."
                ),
        ),
        OracleKind::BroadError => format!(
            "TypeScript preview advisory: broad error evidence does not establish missing discriminator `{discriminator}`; add an exact payload to `toThrow` (string, object, or class reference) and no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available."
        ),
        _ => format!(
            "TypeScript preview advisory: add or strengthen a focused assertion for missing discriminator `{discriminator}`; no actionable repair packet is emitted until verify, receipt, and edit-boundary fields are available."
        ),
    }
}

/// Walk a list of statements (e.g., a function body) and collect every
/// `expect(actual).matcher(...)` expression statement we recognise. Test
/// discriminators are often guarded by setup branches or cleanup blocks, so
/// this recurses through common control-flow bodies while still staying
/// syntax-only and conservative.
///
/// This form recognises only the Jest/Vitest and AVA shapes; the test
/// extractor uses [`collect_assertions_in_statements_with_bindings`] so a
/// file's imported assertion libraries (#4547) are credited too.
#[cfg(test)]
pub(crate) fn collect_expect_assertions_in_statements(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
    source: &SourceText<'_>,
    receiver: Option<&str>,
) -> Vec<TypeScriptAssertion> {
    collect_assertions_in_statements_with_bindings(
        statements,
        source,
        receiver,
        &TypeScriptAssertionBindings::default(),
    )
}

/// Like [`collect_expect_assertions_in_statements`], but also credits
/// `node:assert` / chai calls made through the file's module-level
/// assertion-library `bindings` (#4547).
pub(crate) fn collect_assertions_in_statements_with_bindings(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
    source: &SourceText<'_>,
    receiver: Option<&str>,
    bindings: &TypeScriptAssertionBindings,
) -> Vec<TypeScriptAssertion> {
    let context = AssertionContext { receiver, bindings };
    let mut out = Vec::new();
    for stmt in statements {
        collect_expect_assertions_in_statement(stmt, source, &context, &mut out);
    }
    out
}

/// What a test body's assertions may be made through: the AVA-style callback
/// receiver (`t`) and the file's imported assertion libraries.
pub(crate) struct AssertionContext<'a> {
    receiver: Option<&'a str>,
    bindings: &'a TypeScriptAssertionBindings,
}

/// Try chai's BDD `expect(...).to.equal(...)` shape (only when the file binds
/// chai's `expect`), the Jest `expect(...).matcher(...)` shape, then — when the
/// test exposes an AVA-style callback receiver (`t`) — the `t.is(...)` shape,
/// and finally a `node:assert` / chai `assert` call through an imported
/// binding.
fn assertion_from_expression_any(
    expr: &Expression<'_>,
    source: &SourceText<'_>,
    context: &AssertionContext<'_>,
) -> Option<TypeScriptAssertion> {
    chai_expect_assertion_from_expression(expr, source, context.bindings)
        .or_else(|| expect_assertion_from_expression(expr, source))
        .or_else(|| {
            context
                .receiver
                .and_then(|r| ava_assertion_from_expression(expr, source, r))
        })
        .or_else(|| module_assert_assertion_from_expression(expr, source, context.bindings))
}

pub(crate) fn collect_expect_assertions_in_statement(
    stmt: &Statement<'_>,
    source: &SourceText<'_>,
    context: &AssertionContext<'_>,
    out: &mut Vec<TypeScriptAssertion>,
) {
    match stmt {
        Statement::BlockStatement(block) => {
            collect_expect_assertions_from_statement_vec(&block.body, source, context, out);
        }
        Statement::ExpressionStatement(expr_stmt) => {
            if let Some(assertion) =
                assertion_from_expression_any(&expr_stmt.expression, source, context)
            {
                out.push(assertion);
            }
        }
        Statement::ReturnStatement(return_stmt) => {
            if let Some(argument) = &return_stmt.argument
                && let Some(assertion) = assertion_from_expression_any(argument, source, context)
            {
                out.push(assertion);
            }
        }
        Statement::IfStatement(if_stmt) => {
            collect_expect_assertions_in_statement(&if_stmt.consequent, source, context, out);
            if let Some(alternate) = &if_stmt.alternate {
                collect_expect_assertions_in_statement(alternate, source, context, out);
            }
        }
        Statement::DoWhileStatement(do_while) => {
            collect_expect_assertions_in_statement(&do_while.body, source, context, out);
        }
        Statement::WhileStatement(while_stmt) => {
            collect_expect_assertions_in_statement(&while_stmt.body, source, context, out);
        }
        Statement::ForStatement(for_stmt) => {
            collect_expect_assertions_in_statement(&for_stmt.body, source, context, out);
        }
        Statement::ForInStatement(for_in) => {
            collect_expect_assertions_in_statement(&for_in.body, source, context, out);
        }
        Statement::ForOfStatement(for_of) => {
            collect_expect_assertions_in_statement(&for_of.body, source, context, out);
        }
        Statement::LabeledStatement(labeled) => {
            collect_expect_assertions_in_statement(&labeled.body, source, context, out);
        }
        Statement::SwitchStatement(switch_stmt) => {
            for case in &switch_stmt.cases {
                collect_expect_assertions_from_statement_vec(
                    &case.consequent,
                    source,
                    context,
                    out,
                );
            }
        }
        Statement::TryStatement(try_stmt) => {
            collect_expect_assertions_from_statement_vec(
                &try_stmt.block.body,
                source,
                context,
                out,
            );
            if let Some(handler) = &try_stmt.handler {
                collect_expect_assertions_from_statement_vec(
                    &handler.body.body,
                    source,
                    context,
                    out,
                );
            }
            if let Some(finalizer) = &try_stmt.finalizer {
                collect_expect_assertions_from_statement_vec(&finalizer.body, source, context, out);
            }
        }
        Statement::WithStatement(with_stmt) => {
            collect_expect_assertions_in_statement(&with_stmt.body, source, context, out);
        }
        _ => {}
    }
}

pub(crate) fn collect_expect_assertions_from_statement_vec(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
    source: &SourceText<'_>,
    context: &AssertionContext<'_>,
    out: &mut Vec<TypeScriptAssertion>,
) {
    for stmt in statements {
        collect_expect_assertions_in_statement(stmt, source, context, out);
    }
}

/// Match the simplest `expect(actual).matcher(...)` shape on a top-level
/// expression. Async-aware `.resolves.matcher` / `.rejects.matcher`
/// chains are recognised by checking for one extra member-access hop
/// before the inner `expect(...)` call; the matcher remains the final
/// property name.
pub(crate) fn expect_assertion_from_expression(
    expr: &Expression<'_>,
    source: &SourceText<'_>,
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
    let async_modifier = expect_assertion_chain_modifier(inner);
    let expect_call = expect_call_from_assertion_inner(inner)?;

    let mock_payload = mock_payload_from_assertion(matcher, expect_call, outer_call, source);
    let error_payload = error_payload_from_assertion(matcher, async_modifier, outer_call, source);
    let (oracle_kind, oracle_strength) = if error_payload.is_some() {
        (OracleKind::ExactErrorVariant, OracleStrength::Strong)
    } else {
        oracle_for_matcher(matcher)
    };

    // Oracle metadata (RIPR-SPEC-0085 §PR5).
    // Extract observed_expression from the first argument of `expect(...)`.
    let observed_expression = expect_call
        .arguments
        .first()
        .and_then(|arg| source_text_for_argument(arg, source));

    // Extract expected_value_or_variant from the first matcher argument when it
    // is a concrete resolvable literal. Detect dynamic args to emit the
    // typescript_dynamic_assertion_unresolved limitation.
    let (expected_value_or_variant, has_dynamic_matcher_arg) =
        extract_matcher_expected_value(matcher, &error_payload, outer_call, source);

    let oracle_confidence =
        derive_oracle_confidence(&oracle_strength, &expected_value_or_variant, matcher);

    Some(TypeScriptAssertion {
        matcher: matcher.to_string(),
        argument_count: outer_call.arguments.len(),
        line: source.line_for_offset(outer_call.span.start as usize),
        oracle_kind,
        oracle_strength,
        mock_payload,
        error_payload,
        observed_expression,
        expected_value_or_variant,
        has_dynamic_matcher_arg,
        oracle_confidence,
        rendered_call: None,
    })
}

/// Match an AVA / `node:test`-style assertion `<receiver>.method(actual, expected?)`
/// where `receiver` is the test callback's first parameter (the `t` in
/// `test('name', t => { t.is(...) })`). Requiring the exact receiver identifier
/// keeps this fail-closed: an unrelated `map.is(other)` or `validator.is(v, ty)`
/// is NOT matched, and an unrecognized method returns `None`.
pub(crate) fn ava_assertion_from_expression(
    expr: &Expression<'_>,
    source: &SourceText<'_>,
    receiver: &str,
) -> Option<TypeScriptAssertion> {
    let expr = match expr {
        Expression::AwaitExpression(await_expr) => &await_expr.argument,
        _ => expr,
    };
    let Expression::CallExpression(call) = expr else {
        return None;
    };
    let Expression::StaticMemberExpression(member) = &call.callee else {
        return None;
    };
    let Expression::Identifier(object) = &member.object else {
        return None;
    };
    if object.name.as_str() != receiver {
        return None;
    }
    let method = member.property.name.as_str();
    let (oracle_kind, oracle_strength) = oracle_for_ava_assertion(method);
    if matches!(oracle_kind, OracleKind::Unknown) {
        // Fail closed: only an explicitly recognized AVA assertion counts.
        return None;
    }

    // AVA arg order is (actual, expected[, message]). Observed = actual (arg 0);
    // expected value = arg 1 when it is a concrete literal.
    let observed_expression = call
        .arguments
        .first()
        .and_then(|arg| source_text_for_argument(arg, source));
    let expected_arg = call.arguments.get(1);
    let expected_is_literal = expected_arg.is_some_and(|arg| is_literal_argument(arg));
    let expected_value_or_variant = if expected_is_literal {
        expected_arg.and_then(|arg| source_text_for_argument(arg, source))
    } else {
        None
    };
    let has_dynamic_matcher_arg = expected_arg.is_some() && !expected_is_literal;

    let oracle_confidence =
        derive_oracle_confidence(&oracle_strength, &expected_value_or_variant, method);

    Some(TypeScriptAssertion {
        matcher: method.to_string(),
        argument_count: call.arguments.len(),
        line: source.line_for_offset(call.span.start as usize),
        oracle_kind,
        oracle_strength,
        mock_payload: None,
        error_payload: None,
        observed_expression,
        expected_value_or_variant,
        has_dynamic_matcher_arg,
        oracle_confidence,
        rendered_call: None,
    })
}

/// `node:assert` module specifiers whose exports are assertion functions.
const NODE_ASSERT_MODULES: [&str; 4] = [
    "assert",
    "node:assert",
    "assert/strict",
    "node:assert/strict",
];

/// The chai module specifier.
const CHAI_MODULE: &str = "chai";

/// Which assertion API a binding reaches; it decides how an equality method
/// is graded (#4638 review).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AssertFlavor {
    /// `node:assert/strict`, `assert/strict`, or the `strict` export: `equal`
    /// / `deepEqual` are the strict (`Object.is` / deep-strict) comparisons.
    NodeStrict,
    /// Legacy `node:assert`: `equal` / `deepEqual` compare with loose `==`.
    NodeLegacy,
    /// chai's TDD `assert`: `equal` is loose `==`, `deepEqual` is strict deep
    /// equality.
    Chai,
}

/// Module-level bindings through which a test file reaches an assertion
/// library that is not a test-callback receiver (#4547): `node:assert` (and
/// its `/strict` variant) and chai's `assert` / `expect`.
///
/// Only names the file IMPORTS from one of those modules are recorded — an
/// ESM import or a top-level `require(...)` binding — so a locally declared
/// `assert` / `strictEqual` helper is never credited as an oracle.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TypeScriptAssertionBindings {
    /// Identifiers bound to an assert object (`assert.strictEqual(...)`),
    /// whether the binding is itself the callable `assert(value)`, and the
    /// API it reaches.
    assert_objects: Vec<(String, bool, AssertFlavor)>,
    /// Identifiers bound to a single assert method (`strictEqual`), with the
    /// method they name and the API it comes from.
    assert_methods: Vec<(String, String, AssertFlavor)>,
    /// Identifiers bound to chai's `expect`.
    chai_expects: Vec<String>,
    /// Identifiers bound to the chai module (`chai.expect`, `chai.assert`).
    chai_modules: Vec<String>,
}

impl TypeScriptAssertionBindings {
    /// Collect the assertion-library bindings of a file from its extracted
    /// `imports` plus top-level `require('<module>').<member>` declarations
    /// (`const expect = require('chai').expect`), which the import extractor
    /// does not record.
    pub(crate) fn from_program(
        statements: &oxc_allocator::Vec<'_, Statement<'_>>,
        imports: &[TypeScriptImport],
    ) -> Self {
        let mut bindings = Self::default();
        for import in imports {
            bindings.bind(
                &import.source,
                import.imported.as_deref(),
                import.namespace,
                &import.local,
            );
        }
        for stmt in statements {
            let Statement::VariableDeclaration(decl) = stmt else {
                continue;
            };
            for declarator in &decl.declarations {
                let Some(init) = &declarator.init else {
                    continue;
                };
                let Expression::StaticMemberExpression(member) = init.get_inner_expression() else {
                    continue;
                };
                let Some(module) = require_string_literal_source(&member.object) else {
                    continue;
                };
                let Some(local) = super::owners::binding_identifier_name(&declarator.id) else {
                    continue;
                };
                bindings.bind(&module, Some(member.property.name.as_str()), false, local);
            }
        }
        bindings
    }

    /// Record one `local` name bound to `imported` (`None` for an ESM
    /// namespace import) of `module`.
    fn bind(&mut self, module: &str, imported: Option<&str>, namespace: bool, local: &str) {
        let local = local.to_string();
        if NODE_ASSERT_MODULES.contains(&module) {
            let flavor = if module.ends_with("/strict") {
                AssertFlavor::NodeStrict
            } else {
                AssertFlavor::NodeLegacy
            };
            match imported {
                // `import * as assert from 'node:assert'` — an object, not callable.
                None => self.assert_objects.push((local, false, flavor)),
                // Default export / whole-module `require` are the callable
                // `assert` function of the module.
                Some("default") => self.assert_objects.push((local, true, flavor)),
                // The `strict` export is strict mode from either module.
                Some("strict") => {
                    self.assert_objects
                        .push((local, true, AssertFlavor::NodeStrict));
                }
                Some(method) if !namespace && assert_method_is_recognized(method, flavor) => {
                    self.assert_methods
                        .push((local, method.to_string(), flavor));
                }
                Some(_) => {}
            }
        } else if module == CHAI_MODULE {
            match imported {
                None | Some("default") => self.chai_modules.push(local),
                Some("assert") => self.assert_objects.push((local, true, AssertFlavor::Chai)),
                Some("expect") => self.chai_expects.push(local),
                Some(_) => {}
            }
        }
    }

    /// These bindings minus every local name `is_shadowed` reports as
    /// re-declared closer to the assertion (#4638 review): a test-body
    /// declaration, a test or describe callback parameter, or an enclosing
    /// describe-body declaration. A shadowed name no longer reaches the
    /// imported library, so crediting it would credit a local helper.
    pub(crate) fn without_shadowed(&self, is_shadowed: impl Fn(&str) -> bool) -> Self {
        Self {
            assert_objects: self
                .assert_objects
                .iter()
                .filter(|(local, _, _)| !is_shadowed(local))
                .cloned()
                .collect(),
            assert_methods: self
                .assert_methods
                .iter()
                .filter(|(local, _, _)| !is_shadowed(local))
                .cloned()
                .collect(),
            chai_expects: self
                .chai_expects
                .iter()
                .filter(|local| !is_shadowed(local))
                .cloned()
                .collect(),
            chai_modules: self
                .chai_modules
                .iter()
                .filter(|local| !is_shadowed(local))
                .cloned()
                .collect(),
        }
    }

    /// Whether `name` is the local name of any recorded binding.
    pub(crate) fn binds_local(&self, name: &str) -> bool {
        self.assert_objects
            .iter()
            .any(|(local, _, _)| local == name)
            || self
                .assert_methods
                .iter()
                .any(|(local, _, _)| local == name)
            || self.chai_expects.iter().any(|local| local == name)
            || self.chai_modules.iter().any(|local| local == name)
    }

    fn is_empty(&self) -> bool {
        self.assert_objects.is_empty()
            && self.assert_methods.is_empty()
            && self.chai_expects.is_empty()
            && self.chai_modules.is_empty()
    }

    fn assert_object(&self, name: &str) -> Option<(bool, AssertFlavor)> {
        self.assert_objects
            .iter()
            .find(|(local, _, _)| local == name)
            .map(|(_, callable, flavor)| (*callable, *flavor))
    }

    fn assert_method(&self, name: &str) -> Option<(&str, AssertFlavor)> {
        self.assert_methods
            .iter()
            .find(|(local, _, _)| local == name)
            .map(|(_, method, flavor)| (method.as_str(), *flavor))
    }

    /// The rendered receiver text and API when `expression` is an assert
    /// object: `assert` or `chai.assert`.
    fn assert_object_text(&self, expression: &Expression<'_>) -> Option<(String, AssertFlavor)> {
        match expression {
            Expression::Identifier(ident) => self
                .assert_object(ident.name.as_str())
                .map(|(_, flavor)| (ident.name.to_string(), flavor)),
            Expression::StaticMemberExpression(member)
                if member.property.name.as_str() == "assert" =>
            {
                let Expression::Identifier(module) = &member.object else {
                    return None;
                };
                self.chai_modules
                    .iter()
                    .any(|local| local == module.name.as_str())
                    .then(|| (format!("{}.assert", module.name), AssertFlavor::Chai))
            }
            _ => None,
        }
    }

    /// The rendered callee text when `callee` is chai's `expect`: `expect` or
    /// `chai.expect`.
    fn chai_expect_text(&self, callee: &Expression<'_>) -> Option<String> {
        match callee {
            Expression::Identifier(ident) => self
                .chai_expects
                .iter()
                .any(|local| local == ident.name.as_str())
                .then(|| ident.name.to_string()),
            Expression::StaticMemberExpression(member)
                if member.property.name.as_str() == "expect" =>
            {
                let Expression::Identifier(module) = &member.object else {
                    return None;
                };
                self.chai_modules
                    .iter()
                    .any(|local| local == module.name.as_str())
                    .then(|| format!("{}.expect", module.name))
            }
            _ => None,
        }
    }
}

/// Map a `node:assert` / chai `assert` method to an oracle kind + strength
/// (#4547). Positive strict equality pins the exact value; loose (`==`)
/// equality, negated equality and pattern/containment checks stay
/// relational; truthiness and error assertions stay smoke / broad. Unknown
/// methods — including a method the binding's API does not have — return
/// `Unknown` (fail-closed) and are not credited.
///
/// `equal` / `deepEqual` depend on the API (#4638 review): strict under
/// `node:assert/strict`, loose `==` under legacy `node:assert`; chai's
/// `equal` is loose while its `deepEqual` is strict deep equality.
pub(crate) fn oracle_for_assert_method(
    method: &str,
    flavor: AssertFlavor,
) -> (OracleKind, OracleStrength) {
    let exact = (OracleKind::ExactValue, OracleStrength::Strong);
    let relational = (OracleKind::RelationalCheck, OracleStrength::Weak);
    let smoke = (OracleKind::SmokeOnly, OracleStrength::Smoke);
    let broad_error = (OracleKind::BroadError, OracleStrength::Weak);
    let chai = flavor == AssertFlavor::Chai;
    match method {
        "strictEqual" | "deepStrictEqual" => exact,
        "equal" if flavor == AssertFlavor::NodeStrict => exact,
        "deepEqual" if flavor != AssertFlavor::NodeLegacy => exact,
        "equal" | "deepEqual" | "notStrictEqual" | "notDeepStrictEqual" | "notEqual"
        | "notDeepEqual" | "match" => relational,
        "doesNotMatch" if !chai => relational,
        "notMatch" | "include" | "notInclude" | "lengthOf" if chai => relational,
        "ok" => smoke,
        "isTrue" | "isFalse" | "isOk" | "isNotOk" | "isNull" | "isUndefined" | "isDefined"
            if chai =>
        {
            smoke
        }
        "throws" | "doesNotThrow" => broad_error,
        "rejects" | "doesNotReject" if !chai => broad_error,
        _ => (OracleKind::Unknown, OracleStrength::Unknown),
    }
}

fn assert_method_is_recognized(method: &str, flavor: AssertFlavor) -> bool {
    !matches!(
        oracle_for_assert_method(method, flavor).0,
        OracleKind::Unknown
    )
}

/// Match a `node:assert` / chai `assert` call made through an imported
/// binding (#4547): `assert.strictEqual(actual, expected)`,
/// `chai.assert.equal(...)`, a bare named method `strictEqual(actual,
/// expected)`, or the callable `assert(value)` (smoke). The receiver or callee
/// must be a binding [`TypeScriptAssertionBindings`] recorded from an import,
/// so a same-named local helper is not credited.
pub(crate) fn module_assert_assertion_from_expression(
    expr: &Expression<'_>,
    source: &SourceText<'_>,
    bindings: &TypeScriptAssertionBindings,
) -> Option<TypeScriptAssertion> {
    if bindings.is_empty() {
        return None;
    }
    let expr = match expr {
        Expression::AwaitExpression(await_expr) => &await_expr.argument,
        _ => expr,
    };
    let Expression::CallExpression(call) = expr else {
        return None;
    };
    let (method, flavor, callee_text) = match &call.callee {
        Expression::Identifier(ident) => {
            let name = ident.name.as_str();
            match bindings.assert_object(name) {
                Some((true, flavor)) => ("ok", flavor, name.to_string()),
                _ => {
                    let (method, flavor) = bindings.assert_method(name)?;
                    (method, flavor, name.to_string())
                }
            }
        }
        Expression::StaticMemberExpression(member) => {
            let (receiver, flavor) = bindings.assert_object_text(&member.object)?;
            let method = member.property.name.as_str();
            (method, flavor, format!("{receiver}.{method}"))
        }
        _ => return None,
    };
    let (oracle_kind, oracle_strength) = oracle_for_assert_method(method, flavor);
    if matches!(oracle_kind, OracleKind::Unknown) {
        return None;
    }
    // Argument order is (actual, expected[, message]). Only equality and
    // relational methods carry an expected argument; a truthiness or error
    // assertion's second argument is a message or error matcher.
    let observed_expression = call
        .arguments
        .first()
        .and_then(|arg| source_text_for_argument(arg, source));
    let takes_expected = matches!(
        oracle_kind,
        OracleKind::ExactValue | OracleKind::RelationalCheck
    );
    let expected_arg = call.arguments.get(1).filter(|_| takes_expected);
    let (expected_value_or_variant, has_dynamic_matcher_arg) =
        expected_argument_metadata(expected_arg, source);
    let oracle_confidence =
        derive_oracle_confidence(&oracle_strength, &expected_value_or_variant, method);
    Some(TypeScriptAssertion {
        matcher: method.to_string(),
        argument_count: call.arguments.len(),
        line: source.line_for_offset(call.span.start as usize),
        oracle_kind,
        oracle_strength,
        mock_payload: None,
        error_payload: None,
        observed_expression,
        expected_value_or_variant,
        has_dynamic_matcher_arg,
        oracle_confidence,
        rendered_call: Some(format!("{callee_text}(...)")),
    })
}

/// `(expected literal text, is dynamic)` for an optional expected argument.
fn expected_argument_metadata(
    expected_arg: Option<&Argument<'_>>,
    source: &str,
) -> (Option<String>, bool) {
    match expected_arg {
        Some(arg) if is_literal_argument(arg) => (source_text_for_argument(arg, source), false),
        Some(_) => (None, true),
        None => (None, false),
    }
}

/// chai BDD language chains that carry no assertion of their own, plus the
/// `deep` / `not` flags.
const CHAI_CHAIN_WORDS: [&str; 20] = [
    "to", "be", "been", "is", "that", "which", "and", "has", "have", "with", "at", "of", "same",
    "but", "does", "still", "also", "deep", "strict", "not",
];

/// Map a chai BDD terminal assertion to an oracle (#4547). `is_call`
/// distinguishes a method assertion (`.equal(y)`) from a property assertion
/// (`.true`); `negated` is set when the chain contains `.not`.
fn oracle_for_chai_terminal(
    terminal: &str,
    is_call: bool,
    negated: bool,
) -> (OracleKind, OracleStrength) {
    match (terminal, is_call) {
        ("equal" | "equals" | "eq" | "eql" | "eqls", true) if !negated => {
            (OracleKind::ExactValue, OracleStrength::Strong)
        }
        ("equal" | "equals" | "eq" | "eql" | "eqls", true) => {
            (OracleKind::RelationalCheck, OracleStrength::Weak)
        }
        ("throw" | "throws" | "Throw", true) => (OracleKind::BroadError, OracleStrength::Weak),
        (
            "include" | "includes" | "contain" | "contains" | "match" | "matches" | "above"
            | "below" | "least" | "most" | "lengthOf",
            true,
        ) => (OracleKind::RelationalCheck, OracleStrength::Weak),
        ("true" | "false" | "ok" | "null" | "undefined" | "exist", false) => {
            (OracleKind::SmokeOnly, OracleStrength::Smoke)
        }
        _ => (OracleKind::Unknown, OracleStrength::Unknown),
    }
}

/// Match chai's BDD `expect(actual).to.equal(expected)` shape (#4547),
/// including `.to.deep.equal(...)` / `.to.eql(...)` method assertions and
/// `.to.be.true` property assertions. The `expect` callee must be chai's,
/// bound through an import, so a Jest/Vitest `expect` is never read as chai.
/// Any chain word or terminal outside the recognised table fails closed.
pub(crate) fn chai_expect_assertion_from_expression(
    expr: &Expression<'_>,
    source: &SourceText<'_>,
    bindings: &TypeScriptAssertionBindings,
) -> Option<TypeScriptAssertion> {
    if bindings.chai_expects.is_empty() && bindings.chai_modules.is_empty() {
        return None;
    }
    let expr = match expr {
        Expression::AwaitExpression(await_expr) => &await_expr.argument,
        _ => expr,
    };
    let (terminal_member, terminal_call) = match expr {
        Expression::CallExpression(call) => match &call.callee {
            Expression::StaticMemberExpression(member) => (member, Some(call)),
            _ => return None,
        },
        Expression::StaticMemberExpression(member) => (member, None),
        _ => return None,
    };
    let terminal = terminal_member.property.name.as_str();
    // Walk the language chain back to the `expect(...)` call.
    let mut chain = Vec::new();
    let mut cursor = &terminal_member.object;
    let expect_call = loop {
        match cursor {
            Expression::StaticMemberExpression(member) => {
                let word = member.property.name.as_str();
                if !CHAI_CHAIN_WORDS.contains(&word) {
                    return None;
                }
                chain.push(word);
                cursor = &member.object;
            }
            Expression::CallExpression(call) => break call,
            _ => return None,
        }
    };
    let expect_text = bindings.chai_expect_text(&expect_call.callee)?;
    chain.reverse();
    let negated = chain.contains(&"not");
    let (oracle_kind, oracle_strength) =
        oracle_for_chai_terminal(terminal, terminal_call.is_some(), negated);
    if matches!(oracle_kind, OracleKind::Unknown) {
        return None;
    }
    let observed_expression = expect_call
        .arguments
        .first()
        .and_then(|arg| source_text_for_argument(arg, source));
    let takes_expected = matches!(
        oracle_kind,
        OracleKind::ExactValue | OracleKind::RelationalCheck
    );
    let expected_arg = terminal_call
        .and_then(|call| call.arguments.first())
        .filter(|_| takes_expected);
    let (expected_value_or_variant, has_dynamic_matcher_arg) =
        expected_argument_metadata(expected_arg, source);
    let oracle_confidence =
        derive_oracle_confidence(&oracle_strength, &expected_value_or_variant, terminal);
    let mut rendered = format!("{expect_text}(...)");
    for word in &chain {
        rendered.push('.');
        rendered.push_str(word);
    }
    rendered.push('.');
    rendered.push_str(terminal);
    if terminal_call.is_some() {
        rendered.push_str("(...)");
    }
    let span_start = terminal_call.map_or(terminal_member.span.start, |call| call.span.start);
    Some(TypeScriptAssertion {
        matcher: terminal.to_string(),
        argument_count: terminal_call.map_or(0, |call| call.arguments.len()),
        line: source.line_for_offset(span_start as usize),
        oracle_kind,
        oracle_strength,
        mock_payload: None,
        error_payload: None,
        observed_expression,
        expected_value_or_variant,
        has_dynamic_matcher_arg,
        oracle_confidence,
        rendered_call: Some(rendered),
    })
}

pub(crate) fn expect_assertion_chain_modifier<'a>(inner: &'a Expression<'a>) -> Option<&'a str> {
    match inner {
        Expression::StaticMemberExpression(inner_member) => {
            Some(inner_member.property.name.as_str())
                .filter(|modifier| *modifier == "resolves" || *modifier == "rejects")
        }
        _ => None,
    }
}

pub(crate) fn expect_call_from_assertion_inner<'a>(
    inner: &'a Expression<'a>,
) -> Option<&'a oxc_ast::ast::CallExpression<'a>> {
    match inner {
        // Direct: expect(...).matcher(...)
        Expression::CallExpression(inner_call) if call_expression_is_expect(inner_call) => {
            Some(inner_call)
        }
        // Async chain: expect(...).resolves.matcher(...) etc.
        Expression::StaticMemberExpression(inner_member) => {
            let modifier = inner_member.property.name.as_str();
            if modifier != "resolves" && modifier != "rejects" {
                return None;
            }
            match &inner_member.object {
                Expression::CallExpression(inner_call) if call_expression_is_expect(inner_call) => {
                    Some(inner_call)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

pub(crate) fn call_expression_is_expect(call: &oxc_ast::ast::CallExpression<'_>) -> bool {
    matches!(
        &call.callee,
        Expression::Identifier(ident) if ident.name.as_str() == "expect"
    )
}

pub(crate) fn mock_payload_from_assertion(
    matcher: &str,
    expect_call: &oxc_ast::ast::CallExpression<'_>,
    matcher_call: &oxc_ast::ast::CallExpression<'_>,
    source: &str,
) -> Option<TypeScriptMockPayload> {
    let target = safe_mock_target_text(expect_call.arguments.first()?, source)?;
    match matcher {
        "toHaveBeenCalledWith" if matcher_call.arguments.len() == 1 => {
            let expected =
                safe_mock_expected_argument_text(matcher_call.arguments.first()?, source)?;
            Some(TypeScriptMockPayload {
                target,
                expected,
                kind: TypeScriptMockPayloadKind::CalledWith,
            })
        }
        "toHaveBeenCalledTimes" if matcher_call.arguments.len() == 1 => {
            let expected = safe_mock_call_count_text(matcher_call.arguments.first()?, source)?;
            Some(TypeScriptMockPayload {
                target,
                expected,
                kind: TypeScriptMockPayloadKind::CalledTimes,
            })
        }
        _ => None,
    }
}

pub(crate) fn error_payload_from_assertion(
    matcher: &str,
    async_modifier: Option<&str>,
    matcher_call: &oxc_ast::ast::CallExpression<'_>,
    source: &str,
) -> Option<TypeScriptErrorPayload> {
    match (async_modifier, matcher) {
        (None, "toThrow" | "toThrowError") if matcher_call.arguments.len() == 1 => {
            let arg = matcher_call.arguments.first()?;
            // Priority: string literal > object literal > class/constructor ref.
            // Fail-closed: any form we can't confirm is exact stays None (→ BroadError).
            if let Some(expected) = safe_error_literal_payload_text(arg, source) {
                Some(TypeScriptErrorPayload {
                    expected,
                    kind: TypeScriptErrorPayloadKind::ThrowsLiteral,
                })
            } else if let Some(expected) = safe_error_object_payload_text(arg, source) {
                Some(TypeScriptErrorPayload {
                    expected,
                    kind: TypeScriptErrorPayloadKind::ThrowsObject,
                })
            } else {
                safe_error_class_payload_text(arg, source).map(|expected| TypeScriptErrorPayload {
                    expected,
                    kind: TypeScriptErrorPayloadKind::ThrowsClass,
                })
            }
        }
        (Some("rejects"), "toThrow" | "toThrowError") if matcher_call.arguments.len() == 1 => {
            let arg = matcher_call.arguments.first()?;
            if let Some(expected) = safe_error_literal_payload_text(arg, source) {
                Some(TypeScriptErrorPayload {
                    expected,
                    kind: TypeScriptErrorPayloadKind::RejectsThrowLiteral,
                })
            } else if let Some(expected) = safe_error_object_payload_text(arg, source) {
                Some(TypeScriptErrorPayload {
                    expected,
                    kind: TypeScriptErrorPayloadKind::RejectsThrowObject,
                })
            } else {
                safe_error_class_payload_text(arg, source).map(|expected| TypeScriptErrorPayload {
                    expected,
                    kind: TypeScriptErrorPayloadKind::RejectsThrowClass,
                })
            }
        }
        (Some("rejects"), "toMatchObject") if matcher_call.arguments.len() == 1 => {
            let expected = safe_error_object_payload_text(matcher_call.arguments.first()?, source)?;
            Some(TypeScriptErrorPayload {
                expected,
                kind: TypeScriptErrorPayloadKind::RejectsMatchObject,
            })
        }
        _ => None,
    }
}

pub(crate) fn safe_error_literal_payload_text(arg: &Argument<'_>, source: &str) -> Option<String> {
    matches!(arg, Argument::StringLiteral(_)).then(|| source_text_for_argument(arg, source))?
}

pub(crate) fn safe_error_object_payload_text(arg: &Argument<'_>, source: &str) -> Option<String> {
    match arg {
        Argument::ObjectExpression(object) if safe_mock_expected_object(object) => {
            source_text_for_argument(arg, source)
        }
        _ => None,
    }
}

/// Extract a safe class / constructor reference from a `.toThrow(Arg)` argument.
///
/// Accepts only identifier or dotted-member-path expressions where **every
/// segment** is a safe JavaScript identifier AND **the first segment starts
/// with an ASCII uppercase letter** (the conventional PascalCase signal for
/// error class names: `TypeError`, `AuthError`, `http.NotFoundError`).
///
/// The uppercase-first guard is the fail-closed gate that prevents upgrading
/// `.toThrow(message)` where `message` is a plain camelCase variable.
/// We cannot distinguish a class reference from a variable reference via the
/// AST alone, so we conservatively treat only PascalCase paths as exact
/// constructor assertions.
///
/// Fail-closed: template literals, call expressions, dynamic computed members,
/// lowercase-first paths, and any non-path expression return `None`, keeping
/// the oracle at BroadError (weak strength).
pub(crate) fn safe_error_class_payload_text(arg: &Argument<'_>, source: &str) -> Option<String> {
    let text = source_text_for_argument(arg, source)?;
    if !is_safe_javascript_member_path(&text) {
        return None;
    }
    // Require the first segment to start with an uppercase ASCII letter.
    // This is the conventional PascalCase signal for error class names and
    // prevents camelCase variable references from being promoted.
    let first_segment = text.split('.').next().unwrap_or("");
    first_segment
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_uppercase())
        .then_some(text)
}

pub(crate) fn safe_mock_target_text(arg: &Argument<'_>, source: &str) -> Option<String> {
    let text = source_text_for_argument(arg, source)?;
    is_safe_javascript_member_path(&text).then_some(text)
}

pub(crate) fn safe_mock_expected_argument_text(arg: &Argument<'_>, source: &str) -> Option<String> {
    safe_mock_expected_argument(arg).then(|| source_text_for_argument(arg, source))?
}

pub(crate) fn safe_mock_call_count_text(arg: &Argument<'_>, source: &str) -> Option<String> {
    matches!(arg, Argument::NumericLiteral(_)).then(|| source_text_for_argument(arg, source))?
}

pub(crate) fn source_text_for_argument(arg: &Argument<'_>, source: &str) -> Option<String> {
    let span = arg.span();
    Some(
        source
            .get(span.start as usize..span.end as usize)?
            .trim()
            .to_string(),
    )
}

pub(crate) fn safe_mock_expected_argument(arg: &Argument<'_>) -> bool {
    match arg {
        Argument::StringLiteral(_)
        | Argument::NumericLiteral(_)
        | Argument::BooleanLiteral(_)
        | Argument::NullLiteral(_) => true,
        Argument::ObjectExpression(object) => safe_mock_expected_object(object),
        _ => false,
    }
}

pub(crate) fn safe_mock_expected_object(object: &oxc_ast::ast::ObjectExpression<'_>) -> bool {
    object.properties.iter().all(|property| match property {
        ObjectPropertyKind::ObjectProperty(property) => {
            !property.computed
                && !property.shorthand
                && safe_mock_expected_object_key(&property.key)
                && safe_mock_expected_object_value(&property.value)
        }
        ObjectPropertyKind::SpreadProperty(_) => false,
    })
}

pub(crate) fn safe_mock_expected_object_key(key: &PropertyKey<'_>) -> bool {
    matches!(
        key,
        PropertyKey::StaticIdentifier(_)
            | PropertyKey::StringLiteral(_)
            | PropertyKey::NumericLiteral(_)
    )
}

pub(crate) fn safe_mock_expected_object_value(value: &Expression<'_>) -> bool {
    matches!(
        value,
        Expression::StringLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
    )
}

pub(crate) fn is_safe_javascript_member_path(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty()
        && !text.starts_with('.')
        && !text.ends_with('.')
        && text
            .split('.')
            .all(|segment| is_safe_javascript_identifier(segment.trim()))
}

pub(crate) fn is_safe_javascript_identifier(text: &str) -> bool {
    let mut chars = text.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first == '_' || first == '$' || first.is_ascii_alphabetic())
        && chars.all(is_javascript_identifier_char)
}

/// Returns `true` if the `Argument` is a concrete resolvable literal
/// (string, number, boolean, null, or a safe all-literal object).
/// Returns `false` for variables, function calls, computed expressions,
/// template literals, arrays, etc.
pub(crate) fn is_literal_argument(arg: &Argument<'_>) -> bool {
    match arg {
        Argument::StringLiteral(_)
        | Argument::NumericLiteral(_)
        | Argument::BooleanLiteral(_)
        | Argument::NullLiteral(_) => true,
        Argument::ObjectExpression(object) => safe_mock_expected_object(object),
        // Template literals, identifiers, call expressions, member expressions,
        // unary/binary expressions, array expressions, spread elements, etc.
        // are all treated as dynamic / non-resolvable.
        _ => false,
    }
}

/// Extract the expected value or variant from the first matcher argument.
///
/// Returns `(Some(text), false)` when the argument is a concrete literal.
/// Returns `(None, true)` when the argument exists but is a non-literal
/// dynamic expression (triggers `typescript_dynamic_assertion_unresolved`).
/// Returns `(None, false)` when the matcher takes no argument (e.g.
/// `toThrow()` with no arg, `toBeTruthy()`) or is an error/mock payload
/// (already extracted separately).
pub(crate) fn extract_matcher_expected_value(
    matcher: &str,
    error_payload: &Option<TypeScriptErrorPayload>,
    matcher_call: &oxc_ast::ast::CallExpression<'_>,
    source: &str,
) -> (Option<String>, bool) {
    // Error payloads are already extracted and stored on `error_payload`.
    // Don't double-extract — return (None, false) to avoid confusion.
    if error_payload.is_some() {
        return (None, false);
    }

    // Matchers that take no meaningful scalar argument for oracle metadata.
    // Their "oracle value" is the existence of the call, not an argument text.
    let no_scalar_arg_matchers = [
        "toBeTruthy",
        "toBeFalsy",
        "toBeDefined",
        "toBeUndefined",
        "toBeNull",
        "toBeNaN",
        "toHaveBeenCalled",
        "toMatchSnapshot",
        "toMatchInlineSnapshot",
        "toHaveBeenCalledTimes",
        "toHaveBeenCalledWith",
        "toHaveBeenLastCalledWith",
        "toHaveBeenNthCalledWith",
    ];
    if no_scalar_arg_matchers.contains(&matcher) {
        return (None, false);
    }

    // For the matchers that DO take a scalar expected-value argument.
    let Some(first_arg) = matcher_call.arguments.first() else {
        // No argument — toThrow() with no arg, etc.
        return (None, false);
    };

    if is_literal_argument(first_arg) {
        let text = source_text_for_argument(first_arg, source);
        (text, false)
    } else {
        // Dynamic / non-literal argument — cannot resolve to a concrete value.
        (None, true)
    }
}

/// Derive the oracle confidence level from oracle strength and whether the
/// expected value was resolved to a concrete literal.
pub(crate) fn derive_oracle_confidence(
    strength: &OracleStrength,
    expected_value_or_variant: &Option<String>,
    matcher: &str,
) -> OracleConfidence {
    match strength {
        OracleStrength::Strong => {
            if expected_value_or_variant.is_some() {
                OracleConfidence::High
            } else {
                // Strong matcher but no concrete literal arg (dynamic, or
                // matcher takes no arg like `toBeTruthy`).
                OracleConfidence::Medium
            }
        }
        OracleStrength::Medium => OracleConfidence::Medium,
        OracleStrength::Weak => OracleConfidence::Low,
        OracleStrength::Smoke => OracleConfidence::Low,
        OracleStrength::Unknown | OracleStrength::None => {
            // For error variant: oracle_kind=ExactErrorVariant, strength=Strong,
            // but we check matcher to handle toThrow/toThrowError separately.
            let _ = matcher;
            OracleConfidence::Unknown
        }
    }
}

pub(crate) fn assertion_oracle_text(assertion: &TypeScriptAssertion) -> String {
    if let Some(mock_payload) = &assertion.mock_payload {
        return mock_payload.oracle_text();
    }
    if let Some(error_payload) = &assertion.error_payload {
        return error_payload.oracle_text();
    }
    if let Some(rendered_call) = &assertion.rendered_call {
        return rendered_call.clone();
    }
    if is_execution_context_assertion_matcher(&assertion.matcher) {
        return format!("t.{}(...)", assertion.matcher);
    }
    if matches!(assertion.matcher.as_str(), "toThrow" | "toThrowError")
        && assertion.argument_count == 0
    {
        format!("expect(...).{}()", assertion.matcher)
    } else {
        format!("expect(...).{}(...)", assertion.matcher)
    }
}

pub(crate) fn is_execution_context_assertion_matcher(matcher: &str) -> bool {
    matches!(
        matcher,
        "is" | "not"
            | "equal"
            | "notEqual"
            | "strictEqual"
            | "notStrictEqual"
            | "deepEqual"
            | "notDeepEqual"
            | "true"
            | "false"
            | "truthy"
            | "falsy"
            | "pass"
            | "fail"
            | "assert"
            | "ok"
            | "notOk"
            | "throws"
            | "throwsAsync"
            | "notThrows"
            | "notThrowsAsync"
            | "regex"
            | "notRegex"
            | "like"
            | "notLike"
    )
}

/// Emit the additive oracle metadata evidence lines for an assertion
/// (RIPR-SPEC-0085 §PR5).
///
/// Lines emitted (all additive, none replace existing fields):
/// - `typescript_oracle_observed: <expr>` — the `expect(<expr>)` argument.
/// - `typescript_oracle_expected: <value>` — matcher arg when it is a literal.
/// - `typescript_oracle_confidence: <level>` — derived confidence.
/// - `typescript_oracle_evidence_ref: <file>:<line>` — AST call site.
pub(crate) fn oracle_metadata_evidence_lines(
    assertion: &TypeScriptAssertion,
    test_file: &Path,
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(observed) = &assertion.observed_expression {
        lines.push(format!("typescript_oracle_observed: {observed}"));
    }
    if let Some(expected) = &assertion.expected_value_or_variant {
        lines.push(format!("typescript_oracle_expected: {expected}"));
    }
    lines.push(format!(
        "typescript_oracle_confidence: {}",
        assertion.oracle_confidence.as_str()
    ));
    lines.push(format!(
        "typescript_oracle_evidence_ref: {}:{}",
        normalized_path(test_file),
        assertion.line
    ));
    lines
}

/// Collect additive oracle metadata evidence lines from oracle-eligible
/// related test candidates (RIPR-SPEC-0085 §PR5).
///
/// Emits lines for the single strongest assertion (by `oracle_strength` rank)
/// across all oracle-eligible related tests whose oracle kind can observe the
/// changed probe family (`ts_oracle_kind_matches_seam`, RIPR-SPEC-0104).
/// Heuristic-only candidates are excluded — they are not oracle-eligible and
/// cannot produce oracle metadata.
///
/// The family filter matters because these lines are what the repair-packet
/// projection borrows as its oracle target (RIPR-SPEC-0087 G-C). Borrowing a
/// wrong-family assertion — e.g. `expect(parse('10')).toBe(10)` for a newly
/// added `throw` — produced a "complete" packet whose repair action restated an
/// assertion that cannot observe the change.
///
/// Returns an empty `Vec` when no candidate observes an owner call or no
/// family-matching assertions with metadata exist.
pub(crate) fn collect_oracle_metadata_evidence_lines(
    probe_family: &ProbeFamily,
    candidates: &[TypeScriptRelatedCandidate<'_>],
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> Vec<String> {
    // Candidates observing an owner-name call: trusted relations by
    // construction, plus gate-denied relations whose test still calls the
    // owner by name — assertion classification is independent of relation
    // credit (see `candidate_observes_owner_call`).
    let strongest_assertion_with_file = candidates
        .iter()
        .filter(|candidate| {
            candidate_observes_owner_call(candidate, owner, alias_map, workspace_root)
        })
        .flat_map(|candidate| {
            candidate
                .test
                .assertions
                .iter()
                .map(move |assertion| (assertion, &candidate.test.file))
        })
        .filter(|(assertion, _)| ts_oracle_kind_matches_seam(&assertion.oracle_kind, probe_family))
        .max_by_key(|(assertion, _)| assertion.oracle_strength.rank());

    match strongest_assertion_with_file {
        Some((assertion, file)) => oracle_metadata_evidence_lines(assertion, file),
        None => Vec::new(),
    }
}

/// Pick the highest-rank assertion from a test body. Used to summarise a
/// related test's strongest oracle for the classifier.
pub(crate) fn strongest_assertion(
    assertions: &[TypeScriptAssertion],
) -> Option<&TypeScriptAssertion> {
    assertions
        .iter()
        .max_by_key(|assertion| assertion.oracle_strength.rank())
}

pub(crate) fn related_mock_payload_oracle(related: &[RelatedTest]) -> Option<String> {
    related.iter().find_map(|test| {
        (test.oracle_kind == OracleKind::MockExpectation)
            .then_some(test.oracle.as_deref())
            .flatten()
            .filter(|oracle| !oracle.contains("..."))
            .map(str::to_string)
    })
}

/// Collect the deduplicated set of module paths that any related test
/// file mocks via syntactic `vi.mock("path")` / `jest.mock("path")`.
///
/// Related tests are identified through the same fallback ordering as
/// `find_related_tests`: trusted call/import relations first, then
/// uncertainty-only name/proximity links only when no trusted relation exists.
/// Each selected test's `mocks_in_file` list is contributed once. The
/// classifier uses the resulting list to surface the `mocked_module`
/// static-limit per RIPR-SPEC-0026.
///
/// `workspace_root`, `reexport_index`, and `alias_map` MUST be the same
/// values the classifier passed to `related_test_candidates` for the credited
/// relation set: the mock producer set must be identical to that set, so a
/// cross-package test the relation layer excluded (or an alias the relation
/// layer resolved differently) cannot re-enter through the mock collector
/// and force a wrong actionable `mocked_module` signal on the finding.
pub(crate) fn collect_related_mock_paths(
    owner: &TypeScriptOwner,
    all_tests: &[TypeScriptTest],
    workspace_root: Option<&Path>,
    reexport_index: &ReExportIndex,
    alias_map: Option<&TsAliasMap>,
) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for candidate in
        related_test_candidates(owner, all_tests, workspace_root, reexport_index, alias_map)
    {
        for path in &candidate.test.mocks_in_file {
            if !paths.iter().any(|existing| existing == path) {
                paths.push(path.clone());
            }
        }
    }
    paths
}
