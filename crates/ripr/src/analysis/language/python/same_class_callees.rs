//! Same-class receiver-call facts for Python method owners.
//!
//! Collects `self.helper(` / `cls.helper(` and bound-method aliases
//! (`fn = self.helper` then `fn(`). Nested functions and classes are not
//! walked. `getattr`, `super()`, and non-receiver attributes are ignored.
//! Used only to name a Python transitive-reach limitation (#4765).

use rustpython_parser::ast::{self, Expr, Stmt};

/// Receiver used for same-class calls. `None` for `@staticmethod` and
/// free functions, which have no `self`/`cls` graph.
pub(super) fn method_receiver_name<'a>(
    parameters: &'a [super::PythonParameter],
    decorators: &[String],
) -> Option<&'a str> {
    if decorators
        .iter()
        .any(|decorator| decorator.ends_with("staticmethod"))
    {
        return None;
    }
    parameters.first().map(|parameter| parameter.name.as_str())
}

pub(super) fn same_class_callees_from_body(body: &[Stmt], receiver: Option<&str>) -> Vec<String> {
    let Some(receiver) = receiver else {
        return Vec::new();
    };
    let mut aliases = Vec::new();
    collect_bound_method_aliases(body, receiver, &mut aliases);
    let mut callees = Vec::new();
    collect_receiver_calls(body, receiver, &aliases, &mut callees);
    callees.sort();
    callees.dedup();
    callees
}

fn collect_bound_method_aliases(
    body: &[Stmt],
    receiver: &str,
    aliases: &mut Vec<(String, String)>,
) {
    for stmt in body {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::AsyncFunctionDef(_) | Stmt::ClassDef(_) => {}
            Stmt::Assign(assign) => {
                if let [Expr::Name(target)] = assign.targets.as_slice()
                    && let Some(method) = receiver_attribute_name(assign.value.as_ref(), receiver)
                {
                    aliases.push((target.id.to_string(), method));
                }
            }
            Stmt::AnnAssign(assign) if assign.simple => {
                if let Expr::Name(target) = assign.target.as_ref()
                    && let Some(value) = assign.value.as_deref()
                    && let Some(method) = receiver_attribute_name(value, receiver)
                {
                    aliases.push((target.id.to_string(), method));
                }
            }
            Stmt::If(stmt) => {
                collect_aliases_in_branches(&[&stmt.body, &stmt.orelse], receiver, aliases)
            }
            Stmt::For(stmt) => {
                collect_aliases_in_branches(&[&stmt.body, &stmt.orelse], receiver, aliases)
            }
            Stmt::AsyncFor(stmt) => {
                collect_aliases_in_branches(&[&stmt.body, &stmt.orelse], receiver, aliases)
            }
            Stmt::While(stmt) => {
                collect_aliases_in_branches(&[&stmt.body, &stmt.orelse], receiver, aliases)
            }
            Stmt::With(stmt) => collect_bound_method_aliases(&stmt.body, receiver, aliases),
            Stmt::AsyncWith(stmt) => collect_bound_method_aliases(&stmt.body, receiver, aliases),
            Stmt::Try(stmt) => collect_aliases_in_try(
                &stmt.body,
                &stmt.handlers,
                &stmt.orelse,
                &stmt.finalbody,
                receiver,
                aliases,
            ),
            Stmt::TryStar(stmt) => collect_aliases_in_try(
                &stmt.body,
                &stmt.handlers,
                &stmt.orelse,
                &stmt.finalbody,
                receiver,
                aliases,
            ),
            Stmt::Match(stmt) => {
                for case in &stmt.cases {
                    collect_bound_method_aliases(&case.body, receiver, aliases);
                }
            }
            _ => {}
        }
    }
}

fn collect_aliases_in_branches(
    branches: &[&[Stmt]],
    receiver: &str,
    aliases: &mut Vec<(String, String)>,
) {
    for body in branches {
        collect_bound_method_aliases(body, receiver, aliases);
    }
}

fn collect_aliases_in_try(
    body: &[Stmt],
    handlers: &[ast::ExceptHandler],
    orelse: &[Stmt],
    finalbody: &[Stmt],
    receiver: &str,
    aliases: &mut Vec<(String, String)>,
) {
    collect_bound_method_aliases(body, receiver, aliases);
    for handler in handlers {
        let ast::ExceptHandler::ExceptHandler(handler) = handler;
        collect_bound_method_aliases(&handler.body, receiver, aliases);
    }
    collect_bound_method_aliases(orelse, receiver, aliases);
    collect_bound_method_aliases(finalbody, receiver, aliases);
}

fn collect_receiver_calls(
    body: &[Stmt],
    receiver: &str,
    aliases: &[(String, String)],
    callees: &mut Vec<String>,
) {
    for stmt in body {
        match stmt {
            Stmt::FunctionDef(_) | Stmt::AsyncFunctionDef(_) | Stmt::ClassDef(_) => {}
            Stmt::If(stmt) => {
                collect_calls_from_expr(&stmt.test, receiver, aliases, callees);
                collect_calls_in_branches(&[&stmt.body, &stmt.orelse], receiver, aliases, callees);
            }
            Stmt::For(stmt) => {
                collect_calls_from_expr(&stmt.iter, receiver, aliases, callees);
                collect_calls_in_branches(&[&stmt.body, &stmt.orelse], receiver, aliases, callees);
            }
            Stmt::AsyncFor(stmt) => {
                collect_calls_from_expr(&stmt.iter, receiver, aliases, callees);
                collect_calls_in_branches(&[&stmt.body, &stmt.orelse], receiver, aliases, callees);
            }
            Stmt::While(stmt) => {
                collect_calls_from_expr(&stmt.test, receiver, aliases, callees);
                collect_calls_in_branches(&[&stmt.body, &stmt.orelse], receiver, aliases, callees);
            }
            Stmt::With(stmt) => {
                for item in &stmt.items {
                    collect_calls_from_expr(&item.context_expr, receiver, aliases, callees);
                }
                collect_receiver_calls(&stmt.body, receiver, aliases, callees);
            }
            Stmt::AsyncWith(stmt) => {
                for item in &stmt.items {
                    collect_calls_from_expr(&item.context_expr, receiver, aliases, callees);
                }
                collect_receiver_calls(&stmt.body, receiver, aliases, callees);
            }
            Stmt::Try(stmt) => collect_calls_in_try(
                &stmt.body,
                &stmt.handlers,
                &stmt.orelse,
                &stmt.finalbody,
                receiver,
                aliases,
                callees,
            ),
            Stmt::TryStar(stmt) => collect_calls_in_try(
                &stmt.body,
                &stmt.handlers,
                &stmt.orelse,
                &stmt.finalbody,
                receiver,
                aliases,
                callees,
            ),
            Stmt::Match(stmt) => {
                collect_calls_from_expr(&stmt.subject, receiver, aliases, callees);
                for case in &stmt.cases {
                    collect_receiver_calls(&case.body, receiver, aliases, callees);
                }
            }
            Stmt::Assign(stmt) => collect_calls_from_expr(&stmt.value, receiver, aliases, callees),
            Stmt::AnnAssign(stmt) => {
                if let Some(value) = stmt.value.as_deref() {
                    collect_calls_from_expr(value, receiver, aliases, callees);
                }
            }
            Stmt::AugAssign(stmt) => {
                collect_calls_from_expr(&stmt.value, receiver, aliases, callees);
            }
            Stmt::Return(stmt) => {
                if let Some(value) = stmt.value.as_deref() {
                    collect_calls_from_expr(value, receiver, aliases, callees);
                }
            }
            Stmt::Expr(stmt) => collect_calls_from_expr(&stmt.value, receiver, aliases, callees),
            Stmt::Raise(stmt) => {
                if let Some(exc) = stmt.exc.as_deref() {
                    collect_calls_from_expr(exc, receiver, aliases, callees);
                }
            }
            Stmt::Assert(stmt) => {
                collect_calls_from_expr(&stmt.test, receiver, aliases, callees);
                if let Some(msg) = stmt.msg.as_deref() {
                    collect_calls_from_expr(msg, receiver, aliases, callees);
                }
            }
            _ => {}
        }
    }
}

fn collect_calls_in_branches(
    branches: &[&[Stmt]],
    receiver: &str,
    aliases: &[(String, String)],
    callees: &mut Vec<String>,
) {
    for body in branches {
        collect_receiver_calls(body, receiver, aliases, callees);
    }
}

fn collect_calls_in_try(
    body: &[Stmt],
    handlers: &[ast::ExceptHandler],
    orelse: &[Stmt],
    finalbody: &[Stmt],
    receiver: &str,
    aliases: &[(String, String)],
    callees: &mut Vec<String>,
) {
    collect_receiver_calls(body, receiver, aliases, callees);
    for handler in handlers {
        let ast::ExceptHandler::ExceptHandler(handler) = handler;
        collect_receiver_calls(&handler.body, receiver, aliases, callees);
    }
    collect_receiver_calls(orelse, receiver, aliases, callees);
    collect_receiver_calls(finalbody, receiver, aliases, callees);
}

fn collect_calls_from_expr(
    expr: &Expr,
    receiver: &str,
    aliases: &[(String, String)],
    callees: &mut Vec<String>,
) {
    match expr {
        Expr::Call(call) => {
            if let Some(method) = receiver_attribute_name(call.func.as_ref(), receiver) {
                callees.push(method);
            } else if let Expr::Name(name) = call.func.as_ref() {
                for (alias, method) in aliases {
                    if alias == name.id.as_str() {
                        callees.push(method.clone());
                    }
                }
            }
            collect_calls_from_expr(call.func.as_ref(), receiver, aliases, callees);
            for arg in &call.args {
                collect_calls_from_expr(arg, receiver, aliases, callees);
            }
            for keyword in &call.keywords {
                collect_calls_from_expr(&keyword.value, receiver, aliases, callees);
            }
        }
        Expr::Attribute(attribute) => {
            collect_calls_from_expr(&attribute.value, receiver, aliases, callees);
        }
        Expr::UnaryOp(unary) => collect_calls_from_expr(&unary.operand, receiver, aliases, callees),
        Expr::BinOp(binop) => {
            collect_calls_from_expr(&binop.left, receiver, aliases, callees);
            collect_calls_from_expr(&binop.right, receiver, aliases, callees);
        }
        Expr::BoolOp(boolop) => {
            for value in &boolop.values {
                collect_calls_from_expr(value, receiver, aliases, callees);
            }
        }
        Expr::Compare(compare) => {
            collect_calls_from_expr(&compare.left, receiver, aliases, callees);
            for comparator in &compare.comparators {
                collect_calls_from_expr(comparator, receiver, aliases, callees);
            }
        }
        Expr::IfExp(ifexp) => {
            collect_calls_from_expr(&ifexp.test, receiver, aliases, callees);
            collect_calls_from_expr(&ifexp.body, receiver, aliases, callees);
            collect_calls_from_expr(&ifexp.orelse, receiver, aliases, callees);
        }
        Expr::List(list) => {
            for elt in &list.elts {
                collect_calls_from_expr(elt, receiver, aliases, callees);
            }
        }
        Expr::Tuple(tuple) => {
            for elt in &tuple.elts {
                collect_calls_from_expr(elt, receiver, aliases, callees);
            }
        }
        Expr::Set(set) => {
            for elt in &set.elts {
                collect_calls_from_expr(elt, receiver, aliases, callees);
            }
        }
        Expr::Dict(dict) => {
            for key in dict.keys.iter().flatten() {
                collect_calls_from_expr(key, receiver, aliases, callees);
            }
            for value in &dict.values {
                collect_calls_from_expr(value, receiver, aliases, callees);
            }
        }
        Expr::Subscript(sub) => {
            collect_calls_from_expr(&sub.value, receiver, aliases, callees);
            collect_calls_from_expr(&sub.slice, receiver, aliases, callees);
        }
        Expr::Starred(starred) => {
            collect_calls_from_expr(&starred.value, receiver, aliases, callees)
        }
        Expr::Await(await_expr) => {
            collect_calls_from_expr(&await_expr.value, receiver, aliases, callees);
        }
        Expr::Yield(yield_expr) => {
            if let Some(value) = yield_expr.value.as_deref() {
                collect_calls_from_expr(value, receiver, aliases, callees);
            }
        }
        Expr::YieldFrom(yield_expr) => {
            collect_calls_from_expr(&yield_expr.value, receiver, aliases, callees);
        }
        Expr::ListComp(comp) => collect_calls_from_expr(&comp.elt, receiver, aliases, callees),
        Expr::SetComp(comp) => collect_calls_from_expr(&comp.elt, receiver, aliases, callees),
        Expr::GeneratorExp(comp) => collect_calls_from_expr(&comp.elt, receiver, aliases, callees),
        Expr::DictComp(comp) => {
            collect_calls_from_expr(&comp.key, receiver, aliases, callees);
            collect_calls_from_expr(&comp.value, receiver, aliases, callees);
        }
        // Nested functions are skipped at the statement level; lambdas are the
        // expression equivalent and must not contribute `self.` edges.
        Expr::Lambda(_) => {}
        Expr::NamedExpr(named) => collect_calls_from_expr(&named.value, receiver, aliases, callees),
        Expr::FormattedValue(formatted) => {
            collect_calls_from_expr(&formatted.value, receiver, aliases, callees);
        }
        Expr::JoinedStr(joined) => {
            for value in &joined.values {
                collect_calls_from_expr(value, receiver, aliases, callees);
            }
        }
        _ => {}
    }
}

fn receiver_attribute_name(expr: &Expr, receiver: &str) -> Option<String> {
    let Expr::Attribute(attribute) = expr else {
        return None;
    };
    let Expr::Name(name) = attribute.value.as_ref() else {
        return None;
    };
    (name.id.as_str() == receiver).then(|| attribute.attr.to_string())
}
