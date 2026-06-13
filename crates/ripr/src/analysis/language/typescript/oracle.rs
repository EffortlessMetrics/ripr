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
            "Related test reaches `{owner_name}` but the strongest extracted oracle is `{}`; upgrade by adding an exact-value (`toBe` / `toEqual` / `toStrictEqual`) assertion. TypeScript `toThrow` forms remain broad error evidence until payload inspection lands.",
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
            "TypeScript preview advisory: broad error evidence does not establish missing discriminator `{discriminator}`; no actionable repair packet is emitted until error payload/variant support can name verify, receipt, and edit-boundary fields."
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
pub(crate) fn collect_expect_assertions_in_statements(
    statements: &oxc_allocator::Vec<'_, Statement<'_>>,
    source: &str,
) -> Vec<TypeScriptAssertion> {
    let mut out = Vec::new();
    for stmt in statements {
        collect_expect_assertions_in_statement(stmt, source, &mut out);
    }
    out
}

pub(crate) fn collect_expect_assertions_in_statement(
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

pub(crate) fn collect_expect_assertions_from_statement_vec(
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
pub(crate) fn expect_assertion_from_expression(
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
        line: line_for_offset(source, outer_call.span.start as usize),
        oracle_kind,
        oracle_strength,
        mock_payload,
        error_payload,
        observed_expression,
        expected_value_or_variant,
        has_dynamic_matcher_arg,
        oracle_confidence,
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
            let expected =
                safe_error_literal_payload_text(matcher_call.arguments.first()?, source)?;
            Some(TypeScriptErrorPayload {
                expected,
                kind: TypeScriptErrorPayloadKind::ThrowsLiteral,
            })
        }
        (Some("rejects"), "toThrow" | "toThrowError") if matcher_call.arguments.len() == 1 => {
            let expected =
                safe_error_literal_payload_text(matcher_call.arguments.first()?, source)?;
            Some(TypeScriptErrorPayload {
                expected,
                kind: TypeScriptErrorPayloadKind::RejectsThrowLiteral,
            })
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
    if matches!(assertion.matcher.as_str(), "toThrow" | "toThrowError")
        && assertion.argument_count == 0
    {
        format!("expect(...).{}()", assertion.matcher)
    } else {
        format!("expect(...).{}(...)", assertion.matcher)
    }
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
/// across all oracle-eligible related tests. Heuristic-only candidates are
/// excluded — they are not oracle-eligible and cannot produce oracle metadata.
///
/// Returns an empty `Vec` when there are no oracle-eligible candidates or no
/// assertions with metadata to surface.
pub(crate) fn collect_oracle_metadata_evidence_lines(
    candidates: &[TypeScriptRelatedCandidate<'_>],
) -> Vec<String> {
    // Only oracle-eligible candidates (direct call, imported call, etc.)
    let strongest_assertion_with_file = candidates
        .iter()
        .filter(|c| c.relation.uses_oracle())
        .flat_map(|candidate| {
            candidate
                .test
                .assertions
                .iter()
                .map(move |assertion| (assertion, &candidate.test.file))
        })
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
pub(crate) fn collect_related_mock_paths(
    owner: &TypeScriptOwner,
    all_tests: &[TypeScriptTest],
) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for candidate in related_test_candidates(owner, all_tests, None) {
        for path in &candidate.test.mocks_in_file {
            if !paths.iter().any(|existing| existing == path) {
                paths.push(path.clone());
            }
        }
    }
    paths
}
