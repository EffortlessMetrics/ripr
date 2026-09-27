//! Fail-closed admission for the first derived-local tuple slice.
//!
//! The underlying discriminator understands one bounded closure shape, but its
//! text-shaped initializer and descendant-oracle helpers are intentionally not
//! authority on their own. This layer binds the admitted owner roles and one
//! executed top-level projection oracle before that discriminator may refine
//! evidence. Broader outer-local derivation remains unsupported.

use crate::analysis::classify::ProbeContext;
use crate::analysis::rust_index::find_file_facts;
use crate::domain::{Probe, RelationReason};
use ra_ap_syntax::ast::{HasArgList, HasAttrs, HasName};
use ra_ap_syntax::{AstNode, Edition, SourceFile, SyntaxNode, ast};

pub(super) fn admits(context: &ProbeContext<'_>) -> bool {
    admits_inner(context).unwrap_or(false)
}

fn admits_inner(context: &ProbeContext<'_>) -> Option<bool> {
    let owner = context.owner_fn?;
    let facts = find_file_facts(context.index, &owner.file)?;
    let root = parsed(&facts.source)?;
    let function = named_function(&root, &owner.name)?;
    if !canonical_owner_roles(&function)? {
        return Some(false);
    }
    let result = current_result(&facts.source, &function, context.probe)?;

    let related = context
        .related_tests
        .iter()
        .filter(|(_, reason)| *reason == RelationReason::DirectOwnerCall)
        .collect::<Vec<_>>();
    let [(test, _)] = related.as_slice() else {
        return Some(false);
    };
    let test_facts = find_file_facts(context.index, &test.file)?;
    let test_root = parsed(&test_facts.source)?;
    let test_function = named_function(&test_root, &test.name)?;
    top_level_projection_observes(&test_function, &owner.name, &result)
}

/// First-slice authority is the canonical helper boundary only. Merely finding
/// any owner parameter or any `.contains` call cannot assign semantic roles.
fn canonical_owner_roles(function: &ast::Fn) -> Option<bool> {
    if !canonical_receipt_iteration(function)?
        || !has_exact_parameter(
            function,
            "receipt_request_ids",
            "&BTreeMap<String,Vec<String>>",
        )?
        || !has_exact_parameter(function, "request_set", "&BTreeSet<String>")?
        || !has_exact_parameter(function, "task_id", "&str")?
    {
        return Some(false);
    }

    let source = compact(&function.syntax().text().to_string());
    Some(
        source
            .matches("receipt_request_ids.get(&receipt.id).is_some_and(")
            .count()
            == 1
            && canonical_membership_call_count(function) == 1
            && source
                .matches("lettask_identity_matches=receipt.id==task_id;")
                .count()
                == 1,
    )
}

/// The membership role belongs to the actual `request_set` parameter, so the
/// call is matched as a whole expression. A whole-function substring would
/// over-credit an unrelated collection such as `unrelated_request_set` that
/// merely ends with the same spelling.
fn canonical_membership_call_count(function: &ast::Fn) -> usize {
    function
        .syntax()
        .descendants()
        .filter_map(ast::MethodCallExpr::cast)
        .filter(|call| {
            call.name_ref()
                .is_some_and(|name| name.text() == "contains")
                && compact(&call.syntax().text().to_string())
                    == "request_set.contains(request_id.as_str())"
        })
        .count()
}

/// Bind the closure's source to the owner parameter, not a nearby spelling or
/// an unrelated collection with the same element shape.
fn canonical_receipt_iteration(function: &ast::Fn) -> Option<bool> {
    let mut parameters = function
        .param_list()?
        .params()
        .filter(|parameter| immutable_parameter_name(parameter).as_deref() == Some("receipts"));
    if parameters.next().is_none() || parameters.next().is_some() {
        return Some(false);
    }
    let body = function.body()?.stmt_list()?;
    if body.statements().next().is_some() {
        return Some(false);
    }
    let filtered = method_receiver(&body.tail_expr()?, "collect", 0)?;
    let iterated = method_receiver(&filtered, "filter_map", 1)?;
    let filter_call = ast::MethodCallExpr::cast(filtered.syntax().clone())?;
    let closure = filter_call.arg_list()?.args().next()?;
    if ast::ClosureExpr::cast(closure.syntax().clone()).is_none() {
        return Some(false);
    }
    let receiver = method_receiver(&iterated, "iter", 0)?;
    Some(direct_name(&receiver).as_deref() == Some("receipts"))
}

fn method_receiver(expression: &ast::Expr, name: &str, arity: usize) -> Option<ast::Expr> {
    let method = ast::MethodCallExpr::cast(expression.syntax().clone())?;
    if method.name_ref()?.text() != name || method.arg_list()?.args().count() != arity {
        return None;
    }
    method.receiver()
}

fn has_exact_parameter(
    function: &ast::Fn,
    expected_name: &str,
    expected_type: &str,
) -> Option<bool> {
    let mut matches = function.param_list()?.params().filter(|parameter| {
        immutable_parameter_name(parameter).as_deref() == Some(expected_name)
            && parameter
                .ty()
                .is_some_and(|value| compact(&value.syntax().text().to_string()) == expected_type)
    });
    let present = matches.next().is_some();
    Some(present && matches.next().is_none())
}

fn immutable_parameter_name(parameter: &ast::Param) -> Option<String> {
    let pattern = ast::IdentPat::cast(parameter.pat()?.syntax().clone())?;
    let name = pattern.name()?.text().to_string();
    (pattern.syntax().text().to_string().trim() == name).then_some(name)
}

fn current_result(source: &str, function: &ast::Fn, probe: &Probe) -> Option<String> {
    let after = probe.after.as_deref()?;
    let mut selected = None;
    for arm in function
        .syntax()
        .descendants()
        .filter_map(ast::MatchArm::cast)
    {
        if arm.guard().is_some() || arm.attrs().next().is_some() {
            continue;
        }
        let start = u32::from(arm.syntax().text_range().start()) as usize;
        let line = source
            .get(..start)?
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1;
        if line != probe.location.line
            || !arm_source_matches(&arm, after)?
            || !arm_source_matches(&arm, &probe.expression)?
        {
            continue;
        }
        let result = plain_string(&arm.expr()?)?;
        if selected.replace(result).is_some() {
            return None;
        }
    }
    selected
}

/// The single owner of the derived tuple-arm projection oracle. The sibling
/// `derived` discrimination re-checks this same implementation so no weaker
/// copy can drift back into the evidence path.
pub(super) fn top_level_projection_observes(
    function: &ast::Fn,
    owner: &str,
    expected_result: &str,
) -> Option<bool> {
    let attributes = function.attrs().collect::<Vec<_>>();
    let [attribute] = attributes.as_slice() else {
        return None;
    };
    if attribute.syntax().text() != "#[test]"
        || function.async_token().is_some()
        || function.param_list()?.params().next().is_some()
    {
        return None;
    }

    let body = function.body()?.stmt_list()?;
    if body.tail_expr().is_some() {
        return None;
    }
    let statements = body.statements().collect::<Vec<_>>();
    // Only the containing test's direct return statements bound this scan.
    // A return inside an uninvoked closure or local function does not end it.
    // Keep the full binding/owner scans below: this boundary must not relax
    // their existing conservative shadowing and uniqueness checks.
    let observation_end = statements
        .iter()
        .position(|statement| {
            matches!(statement, ast::Stmt::ExprStmt(statement)
                if matches!(statement.expr(), Some(ast::Expr::ReturnExpr(_))))
        })
        .unwrap_or(statements.len());
    let mut result_binding = None;
    let mut fed_identity = None;
    for (position, statement) in statements.iter().enumerate() {
        let ast::Stmt::LetStmt(binding) = statement else {
            continue;
        };
        let Some(initializer) = binding.initializer() else {
            continue;
        };
        let Some(call) = ast::CallExpr::cast(initializer.syntax().clone()) else {
            continue;
        };
        let Some(callee) = call.expr() else {
            continue;
        };
        if direct_name(&callee).as_deref() != Some(owner) {
            continue;
        }
        if result_binding.is_some() {
            return None;
        }
        result_binding = Some((immutable_binding_name(binding)?, position));
        // The retained receipt identity is read from the owner-call input, so
        // the projection oracle must observe that exact receipt, not merely a
        // relation literal plus cardinality. Bindings after this call cannot
        // feed it.
        fed_identity = Some(fed_receipt_identity(&call, &statements, position)?);
    }
    let (result_binding, owner_position) = result_binding?;
    if owner_position >= observation_end {
        return Some(false);
    }
    let fed_identity = fed_identity?;
    // Equal spelling is not binding identity. Reject shadowing, including
    // destructuring and nested patterns, rather than borrowing their assertions.
    let binding_count = function
        .syntax()
        .descendants()
        .filter_map(ast::IdentPat::cast)
        .filter(|pattern| {
            pattern
                .name()
                .is_some_and(|name| name.text() == result_binding.as_str())
        })
        .count();
    if binding_count != 1 {
        return Some(false);
    }

    let mut length_observed = false;
    let mut result_observed = false;
    let mut identity_observed = false;
    for statement in statements
        .into_iter()
        .take(observation_end)
        .skip(owner_position + 1)
    {
        let ast::Stmt::ExprStmt(statement) = statement else {
            continue;
        };
        let Some(expression) = statement.expr() else {
            continue;
        };
        let Some(macro_expression) = ast::MacroExpr::cast(expression.syntax().clone()) else {
            continue;
        };
        let Some(call) = macro_expression.macro_call() else {
            continue;
        };
        if call.path()?.syntax().text() != "assert_eq" {
            continue;
        }
        let Some((left, right)) = assertion_operands(&call) else {
            continue;
        };
        let left_text = compact(&left.syntax().text().to_string());
        let right_text = compact(&right.syntax().text().to_string());
        let length = format!("{result_binding}.len()");
        let result = format!("{result_binding}[0].1");
        let identity = format!("{result_binding}[0].0.id");
        if (left_text == length && right_text == "1") || (right_text == length && left_text == "1")
        {
            length_observed = true;
        }
        if (left_text == result && plain_string(&right).as_deref() == Some(expected_result))
            || (right_text == result && plain_string(&left).as_deref() == Some(expected_result))
        {
            result_observed = true;
        }
        if (left_text == identity && plain_string(&right).as_deref() == Some(fed_identity.as_str()))
            || (right_text == identity
                && plain_string(&left).as_deref() == Some(fed_identity.as_str()))
        {
            identity_observed = true;
        }
    }

    Some(length_observed && result_observed && identity_observed)
}

/// Read the receipt identity the owner call is actually fed. The first
/// argument is either the input collection itself or one immutable local
/// binding borrowed with `&`; the collection initializer must carry exactly
/// one struct-literal `id` string. Every binding of the borrowed name in the
/// statements before the owner call competes regardless of mut-ness: the
/// call receives the innermost one, so shadowing makes the fed input
/// ambiguous and only an immutable binding can carry the identity its
/// initializer claims. Ambiguity fails closed.
fn fed_receipt_identity(
    call: &ast::CallExpr,
    statements: &[ast::Stmt],
    owner_position: usize,
) -> Option<String> {
    let first_argument = call.arg_list()?.args().next()?;
    let argument_text = compact(&first_argument.syntax().text().to_string());
    let initializer = if let Some(name) = argument_text.strip_prefix('&') {
        if !valid_identifier(name) {
            return None;
        }
        let mut bindings =
            statements[..owner_position]
                .iter()
                .filter_map(|statement| match statement {
                    ast::Stmt::LetStmt(binding)
                        if binding
                            .pat()
                            .is_some_and(|pattern| pattern_binds_name(&pattern, name)) =>
                    {
                        Some(binding)
                    }
                    _ => None,
                });
        let binding = bindings.next()?;
        if bindings.next().is_some() {
            return None;
        }
        // The one binding of this name must be a plain immutable identifier:
        // a `mut` binding could have changed after its identity-bearing
        // initializer, and a destructuring pattern does not make the whole
        // borrowed name the identity-bearing input the doc contract requires.
        immutable_binding_name(binding)?;
        binding.initializer()?
    } else {
        first_argument.clone()
    };
    let identities = id_literals(&initializer);
    let [identity] = identities.as_slice() else {
        return None;
    };
    Some(identity.clone())
}

/// Whether a pattern binds the name anywhere, including inside tuple,
/// struct, or reference destructuring, so shadow counting cannot be
/// bypassed by `let (receipts, other) = ...` re-bindings.
fn pattern_binds_name(pattern: &ast::Pat, name: &str) -> bool {
    pattern.syntax().descendants().any(|node| {
        ast::IdentPat::cast(node)
            .and_then(|ident| ident.name())
            .is_some_and(|binding| binding.text() == name)
    })
}

/// Macro token trees are not parsed as AST, so a `vec![Receipt { id: "..." }]`
/// initializer must be re-parsed before its field initializers carry nodes.
fn id_literals(expression: &ast::Expr) -> Vec<String> {
    let direct = id_literals_from_node(expression.syntax());
    if !direct.is_empty() {
        return direct;
    }
    let Some(macro_expression) = ast::MacroExpr::cast(expression.syntax().clone()) else {
        return Vec::new();
    };
    let Some(token_tree) = macro_expression
        .macro_call()
        .and_then(|call| call.token_tree())
    else {
        return Vec::new();
    };
    let tokens = token_tree.syntax().text().to_string();
    let Some(inner) = tokens
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    else {
        return Vec::new();
    };
    let source = format!("fn __fed() {{ let __receipts = {inner}; }}");
    let Some(root) = parsed(&source) else {
        return Vec::new();
    };
    let Some(function) = named_function(&root, "__fed") else {
        return Vec::new();
    };
    id_literals_from_node(function.syntax())
}

fn id_literals_from_node(node: &SyntaxNode) -> Vec<String> {
    node.descendants()
        .filter_map(ast::RecordExprField::cast)
        .filter(|field| field.name_ref().is_some_and(|name| name.text() == "id"))
        .filter_map(|field| {
            let value = field.expr()?;
            plain_string(&value).or_else(|| {
                let method = ast::MethodCallExpr::cast(value.syntax().clone())?;
                if method.name_ref()?.text() != "to_string" {
                    return None;
                }
                plain_string(&method.receiver()?)
            })
        })
        .collect()
}

fn assertion_operands(call: &ast::MacroCall) -> Option<(ast::Expr, ast::Expr)> {
    let tokens = call.token_tree()?.syntax().text().to_string();
    let inner = tokens.strip_prefix('(')?.strip_suffix(')')?;
    let source = format!("fn __operands() {{ ({inner}) }}");
    let root = parsed(&source)?;
    let function = named_function(&root, "__operands")?;
    let body = function.body()?.stmt_list()?;
    if body.statements().next().is_some() {
        return None;
    }
    let tuple = ast::TupleExpr::cast(body.tail_expr()?.syntax().clone())?;
    let operands = tuple
        .syntax()
        .children()
        .filter_map(ast::Expr::cast)
        .collect::<Vec<_>>();
    let [left, right, ..] = operands.as_slice() else {
        return None;
    };
    Some((left.clone(), right.clone()))
}

fn immutable_binding_name(statement: &ast::LetStmt) -> Option<String> {
    let pattern = ast::IdentPat::cast(statement.pat()?.syntax().clone())?;
    let name = pattern.name()?.text().to_string();
    (pattern.syntax().text().to_string().trim() == name).then_some(name)
}

fn direct_name(expression: &ast::Expr) -> Option<String> {
    let path = ast::PathExpr::cast(expression.syntax().clone())?.path()?;
    let text = path.syntax().text().to_string();
    valid_identifier(&text).then_some(text)
}

fn valid_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn arm_source_matches(arm: &ast::MatchArm, claimed: &str) -> Option<bool> {
    let full = arm.syntax().text().to_string();
    let start = u32::from(arm.syntax().text_range().start());
    let end = u32::from(arm.fat_arrow_token()?.text_range().end());
    let boundary = end.checked_sub(start)? as usize;
    let pattern = full.get(..boundary)?;
    Some(arm_text(&full) == arm_text(claimed) || pattern.trim() == claimed.trim())
}

fn arm_text(text: &str) -> &str {
    text.trim().trim_end_matches(',').trim_end()
}

fn plain_string(expression: &ast::Expr) -> Option<String> {
    let literal = ast::Literal::cast(expression.syntax().clone())?;
    let text = literal.syntax().text().to_string();
    let value = text.strip_prefix('"')?.strip_suffix('"')?;
    (!value.contains(['\\', '"', '\n', '\r'])).then(|| value.to_string())
}

fn compact(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn parsed(source: &str) -> Option<ast::SourceFile> {
    let parse = SourceFile::parse(source, Edition::CURRENT);
    parse.errors().is_empty().then(|| parse.tree())
}

fn named_function(root: &ast::SourceFile, name: &str) -> Option<ast::Fn> {
    let mut functions = root
        .syntax()
        .children()
        .filter_map(ast::Fn::cast)
        .filter(|function| function.name().is_some_and(|value| value.text() == name));
    let function = functions.next()?;
    functions.next().is_none().then_some(function)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iteration_is_admitted(source: &str) -> bool {
        parsed(source)
            .and_then(|root| named_function(&root, "terminalize_proof"))
            .and_then(|function| canonical_receipt_iteration(&function))
            .unwrap_or(false)
    }

    fn projection_is_admitted(body: &str) -> bool {
        let source = format!("#[test] fn projection() {{ {body} }}");
        parsed(&source)
            .and_then(|root| named_function(&root, "projection"))
            .and_then(|function| {
                top_level_projection_observes(&function, "terminalize_proof", "request_identity_v2")
            })
            .unwrap_or(false)
    }

    #[test]
    fn canonical_receipts_parameter_supplies_the_iterator() {
        assert!(iteration_is_admitted(
            "fn terminalize_proof(receipts: &[Receipt]) { receipts.iter().filter_map(|receipt| Some(receipt)).collect() }"
        ));
    }

    #[test]
    fn unrelated_receipts_cannot_supply_the_iterator() {
        assert!(!iteration_is_admitted(
            "fn terminalize_proof(receipts: &[Receipt], unrelated_receipts: &[Receipt]) { unrelated_receipts.iter().filter_map(|receipt| Some(receipt)).collect() }"
        ));
    }

    #[test]
    fn local_or_transformed_receipts_remain_unverified() {
        for source in [
            "fn terminalize_proof(input: &[Receipt]) { let receipts = input; receipts.iter().filter_map(|receipt| Some(receipt)).collect() }",
            "fn terminalize_proof(receipts: &[Receipt]) { receipts.iter().skip(1).filter_map(|receipt| Some(receipt)).collect() }",
            "fn terminalize_proof(receipts: &[Receipt]) { receipts.iter().filter_map(adapter(|receipt| Some(receipt))).collect() }",
        ] {
            assert!(!iteration_is_admitted(source), "{source}");
        }
    }

    #[test]
    fn assertions_after_the_unique_owner_result_are_admitted() {
        assert!(projection_is_admitted(
            r#"let receipts = vec![Receipt { id: "receipt-1".to_string() }];
            let terminal = terminalize_proof(&receipts);
            assert_eq!(terminal.len(), 1);
            assert_eq!(terminal[0].0.id, "receipt-1");
            assert_eq!(terminal[0].1, "request_identity_v2");"#
        ));
    }

    #[test]
    fn identity_assertion_must_match_the_fed_receipt() {
        assert!(!projection_is_admitted(
            r#"let receipts = vec![Receipt { id: "input-receipt".to_string() }];
            let terminal = terminalize_proof(&receipts);
            assert_eq!(terminal.len(), 1);
            assert_eq!(terminal[0].0.id, "receipt-1");
            assert_eq!(terminal[0].1, "request_identity_v2");"#
        ));
    }

    #[test]
    fn cardinality_and_relation_without_identity_remain_unverified() {
        assert!(!projection_is_admitted(
            r#"let receipts = vec![Receipt { id: "receipt-1".to_string() }];
            let terminal = terminalize_proof(&receipts);
            assert_eq!(terminal.len(), 1);
            assert_eq!(terminal[0].1, "request_identity_v2");"#
        ));
    }

    #[test]
    fn shadowed_owner_result_cannot_borrow_assertions() {
        for shadow in [
            r#"let terminal = [(0, "request_identity_v2")];"#,
            r#"let (terminal,) = ([(0, "request_identity_v2")],);"#,
        ] {
            let body = format!(
                r#"let receipts = vec![Receipt {{ id: "receipt-1".to_string() }}];
                let terminal = terminalize_proof(&receipts);
                {shadow}
                assert_eq!(terminal.len(), 1);
                assert_eq!(terminal[0].0.id, "receipt-1");
                assert_eq!(terminal[0].1, "request_identity_v2");"#
            );
            assert!(!projection_is_admitted(&body), "{shadow}");
        }
    }

    #[test]
    fn mutable_receipt_binding_remains_unverified() {
        assert!(!projection_is_admitted(
            r#"let mut receipts = vec![Receipt { id: "receipt-1".to_string() }];
            let terminal = terminalize_proof(&receipts);
            assert_eq!(terminal.len(), 1);
            assert_eq!(terminal[0].0.id, "receipt-1");
            assert_eq!(terminal[0].1, "request_identity_v2");"#
        ));
    }

    #[test]
    fn mutable_shadow_over_the_fed_receipt_binding_remains_unverified() {
        // The owner call receives the mutable shadow, not the earlier
        // immutable initializer, so counting every binding of the name makes
        // the fed input ambiguous and the whole lookup fails closed.
        assert!(!projection_is_admitted(
            r#"let receipts = vec![Receipt { id: "receipt-1".to_string() }];
            let mut receipts = vec![Receipt { id: "receipt-1".to_string() }];
            let terminal = terminalize_proof(&receipts);
            assert_eq!(terminal.len(), 1);
            assert_eq!(terminal[0].0.id, "receipt-1");
            assert_eq!(terminal[0].1, "request_identity_v2");"#
        ));
    }

    #[test]
    fn destructuring_shadow_over_the_fed_receipt_name_is_ambiguous() {
        // A tuple destructuring re-binding of the name is still a competing
        // binding of `receipts`: the owner call would receive the destructured
        // shadow, so counting must see it and fail closed instead of
        // crediting the earlier immutable initializer.
        assert!(!projection_is_admitted(
            r#"let receipts = vec![Receipt { id: "receipt-1".to_string() }];
            let (receipts, other) = (vec![Receipt { id: "input-receipt".to_string() }], 1);
            let terminal = terminalize_proof(&receipts);
            assert_eq!(terminal.len(), 1);
            // The assertion must carry the earlier initializer's identity: a
            // lookup that ignores the destructuring shadow would derive
            // "receipt-1" and admit, so only the shadow count can reject.
            assert_eq!(terminal[0].0.id, "receipt-1");
            assert_eq!(terminal[0].1, "request_identity_v2");"#
        ));
    }

    #[test]
    fn shadow_after_the_owner_call_leaves_the_fed_identity_intact() {
        // Bindings declared after the owner call cannot feed it, so the
        // earlier immutable initializer stays the exact fed input; counting
        // the whole body instead would over-reject this honest shape.
        assert!(projection_is_admitted(
            r#"let receipts = vec![Receipt { id: "receipt-1".to_string() }];
            let terminal = terminalize_proof(&receipts);
            let receipts = vec![Receipt { id: "input-receipt".to_string() }];
            assert_eq!(terminal.len(), 1);
            assert_eq!(terminal[0].0.id, "receipt-1");
            assert_eq!(terminal[0].1, "request_identity_v2");"#
        ));
    }

    #[test]
    fn assertions_before_the_owner_call_are_not_its_observation() {
        // The complete oracle evidence is present, but every observation
        // precedes the owner call, so statement order is the only rejection
        // reason and the fed-identity requirement cannot mask it.
        assert!(!projection_is_admitted(
            r#"let receipts = vec![Receipt { id: "receipt-1".to_string() }];
            assert_eq!(terminal.len(), 1);
            assert_eq!(terminal[0].0.id, "receipt-1");
            assert_eq!(terminal[0].1, "request_identity_v2");
            let terminal = terminalize_proof(&receipts);"#
        ));
    }
}

#[cfg(test)]
#[path = "derived_admission/return_boundary_tests.rs"]
mod return_boundary_tests;
