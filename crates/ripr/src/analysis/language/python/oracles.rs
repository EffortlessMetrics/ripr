use super::source_utils::{line_for_range_start, text_for_range};
use super::{PythonAssertion, PythonOracleShape, expr_full_name};
use crate::domain::{OracleKind, OracleStrength};
use rustpython_parser::ast::{self, Expr, Stmt};

pub(super) fn collect_assertions_from_statements(
    statements: &[Stmt],
    source: &str,
) -> Vec<PythonAssertion> {
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
