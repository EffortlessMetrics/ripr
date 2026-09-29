//! Same-module entry points that reach a changed top-level function.
//!
//! A test cannot call a module-private helper, and it often calls an exported
//! function only through a value a same-module factory built:
//!
//! ```ts
//! function _defu(base, defaults) { /* changed */ }
//! export function createDefu(merger) {
//!   return (...args) => args.reduce((p, c) => _defu(p, c, "", merger), {});
//! }
//! export const defu = createDefu();
//! ```
//!
//! A test calling `defu(...)` runs `_defu` through the factory's returned
//! closure. This module records, per top-level function owner, the exported
//! names whose code reaches it through a bounded same-module call graph, so the
//! relation layer can relate tests of those entries to the owner. The graph is
//! syntax-only and fail-closed: an edge exists only for a bare call to a
//! top-level declaration name whose every mention in the enclosing function is
//! a bare call, a function's own edges exclude the functions it returns, and a
//! factory product reaches only what the factory's directly returned function
//! calls. Reach through an entry is weaker than a direct owner call: it is
//! admitted only when no test calls the owner itself, and the classifier never
//! promotes it to `exposed`.

use super::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Maximum call edges followed from an exported entry to the owner. Matches
/// the Rust helper-owner graph bound (`HELPER_OWNER_CALL_GRAPH_MAX_HOPS`).
const MODULE_ENTRY_MAX_HOPS: usize = 3;

/// An exported name of the owner's module whose code reaches the owner.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TypeScriptModuleEntry {
    /// Exported name a test imports (`default` for the default export).
    pub(crate) name: String,
    /// `true` when this entry is the module's default export, so a default
    /// import under any local name binds it.
    pub(crate) exported_as_default: bool,
}

/// One top-level declaration of the module.
#[derive(Default)]
struct ModuleNode<'s> {
    /// Function texts whose bare calls count as this node's calls.
    call_texts: Vec<CallText<'s>>,
    /// Factory whose returned function this node's value is (`const p = f()`).
    factory: Option<String>,
    /// Function texts the declaration returns directly, when it is a factory.
    returned_texts: Vec<CallText<'s>>,
}

/// Code whose bare calls count, plus the enclosing function text that decides
/// whether a name is rebound there.
struct CallText<'s> {
    /// The calling code. For a function's own node, the directly returned
    /// functions are blanked out: they run only when the product is called.
    calls: String,
    /// The whole enclosing function (the factory, for a returned closure), so
    /// its parameters and locals shadow the top-level binding too.
    scope: &'s str,
}

/// Exported entries per top-level owner name.
pub(crate) fn module_entries_by_owner(
    statements: &[Statement<'_>],
    source: &str,
) -> BTreeMap<String, Vec<TypeScriptModuleEntry>> {
    let mut nodes: BTreeMap<String, ModuleNode<'_>> = BTreeMap::new();
    let mut exports: BTreeMap<String, BTreeSet<TypeScriptModuleEntry>> = BTreeMap::new();
    for statement in statements {
        collect_statement(statement, source, &mut nodes, &mut exports);
    }
    let names: BTreeSet<&str> = nodes.keys().map(String::as_str).collect();
    let callees: BTreeMap<&str, BTreeSet<&str>> = nodes
        .iter()
        .map(|(name, node)| (name.as_str(), node_callees(node, &nodes, &names)))
        .collect();
    let mut entries: BTreeMap<String, Vec<TypeScriptModuleEntry>> = BTreeMap::new();
    for (local, exported) in &exports {
        for reached in reachable_within_hops(local, &callees) {
            if reached == local.as_str() {
                continue;
            }
            let owner_entries = entries.entry(reached.to_string()).or_default();
            for entry in exported {
                if !owner_entries.contains(entry) {
                    owner_entries.push(entry.clone());
                }
            }
        }
    }
    for owner_entries in entries.values_mut() {
        owner_entries.sort();
    }
    entries
}

fn collect_statement<'s>(
    statement: &Statement<'_>,
    source: &'s str,
    nodes: &mut BTreeMap<String, ModuleNode<'s>>,
    exports: &mut BTreeMap<String, BTreeSet<TypeScriptModuleEntry>>,
) {
    match statement {
        Statement::FunctionDeclaration(func) => {
            collect_function(func, source, nodes);
        }
        Statement::VariableDeclaration(decl) => {
            collect_variables(decl, source, nodes);
        }
        Statement::ExportNamedDeclaration(export) => {
            if export.export_kind == ImportOrExportKind::Type {
                return;
            }
            match export.declaration.as_ref() {
                Some(Declaration::FunctionDeclaration(func)) => {
                    if let Some(name) = collect_function(func, source, nodes) {
                        record_export(exports, &name, &name);
                    }
                }
                Some(Declaration::VariableDeclaration(decl)) => {
                    for name in collect_variables(decl, source, nodes) {
                        record_export(exports, &name, &name);
                    }
                }
                Some(_) => {}
                // `export { local as exported }` without a source names local
                // bindings; a re-export from another module (`from "./x"`)
                // binds nothing of this module.
                None if export.source.is_none() => {
                    for specifier in &export.specifiers {
                        if specifier.export_kind == ImportOrExportKind::Type {
                            continue;
                        }
                        let (Some(local), Some(exported)) = (
                            module_export_name_text(&specifier.local),
                            module_export_name_text(&specifier.exported),
                        ) else {
                            continue;
                        };
                        record_export(exports, &local, &exported);
                    }
                }
                None => {}
            }
        }
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                // An anonymous default function is a node of its own; no
                // declaration can be named `default`, so the key is free.
                let name = func
                    .id
                    .as_ref()
                    .map_or_else(|| "default".to_string(), |id| id.name.to_string());
                if collect_function_as(func, &name, source, nodes) {
                    record_export(exports, &name, "default");
                }
            }
            ExportDefaultDeclarationKind::ArrowFunctionExpression(arrow) => {
                collect_arrow_as(arrow, "default", source, nodes);
                record_export(exports, "default", "default");
            }
            ExportDefaultDeclarationKind::Identifier(ident) => {
                record_export(exports, ident.name.as_str(), "default");
            }
            _ => {}
        },
        _ => {}
    }
}

fn record_export(
    exports: &mut BTreeMap<String, BTreeSet<TypeScriptModuleEntry>>,
    local: &str,
    exported: &str,
) {
    exports
        .entry(local.to_string())
        .or_default()
        .insert(TypeScriptModuleEntry {
            name: exported.to_string(),
            exported_as_default: exported == "default",
        });
}

fn collect_function<'s>(
    func: &Function<'_>,
    source: &'s str,
    nodes: &mut BTreeMap<String, ModuleNode<'s>>,
) -> Option<String> {
    let name = func.id.as_ref()?.name.to_string();
    collect_function_as(func, &name, source, nodes).then_some(name)
}

/// Record `func` as node `name`; `false` for an overload signature, which
/// carries no body.
fn collect_function_as<'s>(
    func: &Function<'_>,
    name: &str,
    source: &'s str,
    nodes: &mut BTreeMap<String, ModuleNode<'s>>,
) -> bool {
    let Some(body) = func.body.as_ref() else {
        return false;
    };
    let returned = returned_functions(&body.statements);
    add_function_node(name, func.span, &returned, source, nodes);
    true
}

fn collect_arrow_as<'s>(
    arrow: &ArrowFunctionExpression<'_>,
    name: &str,
    source: &'s str,
    nodes: &mut BTreeMap<String, ModuleNode<'s>>,
) {
    let returned = if arrow.expression {
        // `(m) => (...args) => ...`: the expression body is the returned value.
        arrow_expression_body(arrow)
            .and_then(function_span)
            .into_iter()
            .collect()
    } else {
        returned_functions(&arrow.body.statements)
    };
    add_function_node(name, arrow.span, &returned, source, nodes);
}

/// A function node: its own calls exclude the functions it returns, which
/// become the calls of any value it builds.
fn add_function_node<'s>(
    name: &str,
    span: oxc_span::Span,
    returned: &[oxc_span::Span],
    source: &'s str,
    nodes: &mut BTreeMap<String, ModuleNode<'s>>,
) {
    let scope = span_text(source, span);
    let mut own = scope.to_string();
    for inner in returned {
        let start = inner.start.saturating_sub(span.start) as usize;
        let end = inner.end.saturating_sub(span.start) as usize;
        if let Some(range) = own.get(start..end) {
            let blank = " ".repeat(range.len());
            own.replace_range(start..end, &blank);
        }
    }
    let node = nodes.entry(name.to_string()).or_default();
    node.call_texts.push(CallText { calls: own, scope });
    node.returned_texts
        .extend(returned.iter().map(|inner| CallText {
            calls: span_text(source, *inner).to_string(),
            scope,
        }));
}

/// Top-level `const`/`let`/`var` declarators this module can follow; returns
/// the bound names that became graph nodes.
fn collect_variables<'s>(
    decl: &VariableDeclaration<'_>,
    source: &'s str,
    nodes: &mut BTreeMap<String, ModuleNode<'s>>,
) -> Vec<String> {
    let mut names = Vec::new();
    for declarator in &decl.declarations {
        let Some(name) = binding_identifier_name(&declarator.id) else {
            continue;
        };
        let Some(init) = declarator.init.as_ref() else {
            continue;
        };
        match unwrap_type_wrappers(init) {
            Expression::ArrowFunctionExpression(arrow) => {
                collect_arrow_as(arrow, name, source, nodes);
                names.push(name.to_string());
            }
            Expression::FunctionExpression(func) => {
                names.extend(
                    collect_function_as(func, name, source, nodes).then(|| name.to_string()),
                );
            }
            Expression::CallExpression(call) => {
                let Expression::Identifier(callee) = unwrap_type_wrappers(&call.callee) else {
                    continue;
                };
                nodes.entry(name.to_string()).or_default().factory = Some(callee.name.to_string());
                names.push(name.to_string());
            }
            _ => {}
        }
    }
    names
}

/// Spans of the function expressions the body returns from its own top-level
/// `return` statements. Nested returns (inside `if` or loops) are not followed.
fn returned_functions(statements: &[Statement<'_>]) -> Vec<oxc_span::Span> {
    statements
        .iter()
        .filter_map(|statement| match statement {
            Statement::ReturnStatement(ret) => ret.argument.as_ref(),
            _ => None,
        })
        .filter_map(function_span)
        .collect()
}

fn function_span(expression: &Expression<'_>) -> Option<oxc_span::Span> {
    match unwrap_type_wrappers(expression) {
        Expression::ArrowFunctionExpression(arrow) => Some(arrow.span),
        Expression::FunctionExpression(func) => Some(func.span),
        _ => None,
    }
}

fn arrow_expression_body<'a>(arrow: &'a ArrowFunctionExpression<'a>) -> Option<&'a Expression<'a>> {
    match arrow.body.statements.first()? {
        Statement::ExpressionStatement(statement) => Some(&statement.expression),
        _ => None,
    }
}

/// Strip `as T`, `satisfies T`, `x!` and parentheses, which do not change the
/// runtime value.
fn unwrap_type_wrappers<'a>(expression: &'a Expression<'a>) -> &'a Expression<'a> {
    match expression {
        Expression::TSAsExpression(inner) => unwrap_type_wrappers(&inner.expression),
        Expression::TSSatisfiesExpression(inner) => unwrap_type_wrappers(&inner.expression),
        Expression::TSNonNullExpression(inner) => unwrap_type_wrappers(&inner.expression),
        Expression::ParenthesizedExpression(inner) => unwrap_type_wrappers(&inner.expression),
        other => other,
    }
}

fn span_text(source: &str, span: oxc_span::Span) -> &str {
    source
        .get(span.start as usize..span.end as usize)
        .unwrap_or_default()
}

/// Top-level names a node calls. A factory product calls what its factory's
/// returned function calls.
fn node_callees<'n>(
    node: &ModuleNode<'_>,
    nodes: &BTreeMap<String, ModuleNode<'_>>,
    names: &BTreeSet<&'n str>,
) -> BTreeSet<&'n str> {
    let texts: Vec<&CallText<'_>> = match node.factory.as_deref() {
        Some(factory) => nodes
            .get(factory)
            .map(|factory_node| factory_node.returned_texts.iter().collect())
            .unwrap_or_default(),
        None => node.call_texts.iter().collect(),
    };
    // Tokenize each text once and confirm only top-level names it mentions,
    // so a module with many declarations is not rescanned once per name.
    let mut callees = BTreeSet::new();
    for text in texts {
        for token in text
            .calls
            .split(|ch: char| !is_javascript_identifier_char(ch))
            .filter(|token| !token.is_empty())
            .collect::<BTreeSet<_>>()
        {
            if let Some(&name) = names.get(token)
                && text_calls_top_level(text, name)
            {
                callees.insert(name);
            }
        }
    }
    callees
}

/// A bare call to `name` that the enclosing function cannot have rebound.
/// Fail-closed: every mention of `name` in the scope must be a bare call. Any
/// other mention (a parameter, including destructured and nested-callback
/// parameters, an assignment, a `function name` declaration, a value passed
/// along) may bind or alias it, so no edge is recorded.
fn text_calls_top_level(text: &CallText<'_>, name: &str) -> bool {
    contains_call_name(&text.calls, name) && every_mention_is_a_bare_call(text.scope, name)
}

fn every_mention_is_a_bare_call(scope: &str, name: &str) -> bool {
    scope.match_indices(name).all(|(idx, _)| {
        let before = &scope[..idx];
        let after = &scope[idx + name.len()..];
        let bounded_before = before
            .chars()
            .next_back()
            .is_none_or(|ch| !is_javascript_identifier_char(ch));
        let bounded_after = after
            .chars()
            .next()
            .is_none_or(|ch| !is_javascript_identifier_char(ch));
        if !(bounded_before && bounded_after) {
            // Part of a longer identifier: not a mention of `name`.
            return true;
        }
        let member = before.trim_end().ends_with('.');
        let declared = before
            .trim_end()
            .strip_suffix("function")
            .is_some_and(|rest| {
                rest.chars()
                    .next_back()
                    .is_none_or(|ch| !is_javascript_identifier_char(ch))
            });
        let rest = after.trim_start();
        let called = rest.starts_with('(') || rest.starts_with("?.(");
        // `x.name(...)` is a member call, which is another binding entirely.
        member || (called && !declared)
    })
}

fn reachable_within_hops<'n>(
    start: &'n str,
    callees: &BTreeMap<&'n str, BTreeSet<&'n str>>,
) -> BTreeSet<&'n str> {
    let mut seen: BTreeSet<&str> = BTreeSet::from([start]);
    let mut queue: VecDeque<(&str, usize)> = VecDeque::from([(start, 0)]);
    while let Some((name, hops)) = queue.pop_front() {
        if hops == MODULE_ENTRY_MAX_HOPS {
            continue;
        }
        for &callee in callees.get(name).into_iter().flatten() {
            if seen.insert(callee) {
                queue.push_back((callee, hops + 1));
            }
        }
    }
    seen
}
