//! Bounded execution context for owner-return pins, not a Rust resolver.
//!
//! Oracle extraction intentionally retains deferred assertions. This query
//! admits only uniquely identified assertions on ordinary statement paths,
//! optionally through one syntactically bound, directly invoked closure.

use super::parse_clean_source_file;
use super::ra::{LineIndex, slice_macro_call_text, slice_text};
use crate::analysis::facts::cfg_predicates::attribute_test_build_availability;
use ra_ap_syntax::{
    AstNode, SyntaxNode,
    ast::{self, HasArgList, HasAttrs, HasName},
};
use std::collections::{BTreeMap, BTreeSet};

type AssertionKey = (usize, String);
type FunctionKey = (usize, usize, String);

#[derive(Clone, Debug, Default)]
pub(crate) struct OwnerPinAssertions {
    functions: BTreeMap<FunctionKey, FunctionAssertions>,
    module_declarations: BTreeMap<(usize, String), bool>,
}

#[derive(Clone, Debug, Default)]
struct FunctionAssertions {
    body: String,
    assertions: BTreeSet<AssertionKey>,
    macros: BTreeSet<String>,
}

impl OwnerPinAssertions {
    pub(crate) fn admits_module_declaration(&self, line: usize, declaration: &str) -> bool {
        self.module_declarations
            .get(&(line, declaration.to_string()))
            .copied()
            .unwrap_or(false)
    }

    pub(crate) fn admits(
        &self,
        function: (usize, usize, &str),
        body: &str,
        assertion: (usize, &str),
        ambiguous_macros: &BTreeSet<String>,
    ) -> bool {
        self.functions
            .get(&(function.0, function.1, function.2.to_string()))
            .is_some_and(|facts| {
                facts.body == body
                    && facts.macros.is_disjoint(ambiguous_macros)
                    && facts
                        .assertions
                        .contains(&(assertion.0, assertion.1.to_string()))
            })
    }
}

/// Visible bindings that may shadow trusted macros. Visibility and namespace
/// are intentionally not resolved; each test consults only names it uses.
/// Unknown macro imports affect all trusted names, including cross-file scope.
pub(crate) fn trusted_macro_binding_ambiguities(
    source: &str,
    packages: &BTreeSet<String>,
    trusted: &[&str],
) -> BTreeSet<String> {
    let mut ambiguous = BTreeSet::new();
    if !source.contains("macro")
        && !source.contains("use")
        && !source.contains("no_implicit_prelude")
        && !source.contains('!')
    {
        return ambiguous;
    }
    let all = || trusted.iter().map(|name| (*name).to_string()).collect();
    let Some(parse) = parse_clean_source_file(source) else {
        return all();
    };
    for node in parse.tree().syntax().descendants() {
        let definition = ast::MacroRules::cast(node.clone())
            .and_then(|item| item.name())
            .or_else(|| ast::MacroDef::cast(node.clone()).and_then(|item| item.name()));
        if let Some(name) = definition {
            let name = name.text().to_string();
            let name = name.trim_start_matches("r#");
            if trusted.contains(&name) {
                ambiguous.insert(name.to_string());
            }
        }
        if let Some(attr) = ast::Attr::cast(node.clone())
            && attr
                .syntax()
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .any(|token| matches!(token.text(), "macro_use" | "no_implicit_prelude"))
        {
            return all();
        }
        if let Some(call) = ast::MacroCall::cast(node.clone())
            && call
                .path()
                .is_some_and(|path| !is_trusted_macro(&path.syntax().text().to_string(), trusted))
            && let Some(tree) = call.token_tree()
        {
            for token in tree
                .syntax()
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
            {
                let name = token.text().trim_start_matches("r#");
                if trusted.contains(&name) {
                    ambiguous.insert(name.to_string());
                }
            }
        }
        if let Some(import) = ast::Use::cast(node) {
            let Some(tree) = import.use_tree() else {
                return all();
            };
            let root = tree
                .path()
                .map(|path| path.syntax().text().to_string())
                .unwrap_or_default();
            let root = root
                .trim_start_matches("::")
                .split("::")
                .next()
                .unwrap_or("")
                .trim();
            let own = matches!(root, "crate" | "self" | "super")
                || packages
                    .iter()
                    .any(|package| package.replace('-', "_") == root);
            for item in tree.syntax().descendants().filter_map(ast::UseTree::cast) {
                if item.star_token().is_some() && !own {
                    return all();
                }
                let name = if let Some(rename) = item.rename() {
                    rename.name().map(|name| name.text().to_string())
                } else if item.use_tree_list().is_none() {
                    item.path()
                        .and_then(|path| path.segment())
                        .and_then(|segment| segment.name_ref())
                        .map(|name| name.text().to_string())
                } else {
                    None
                };
                if let Some(name) = name {
                    let name = name.trim_start_matches("r#");
                    if trusted.contains(&name) {
                        ambiguous.insert(name.to_string());
                    }
                }
            }
        }
    }
    ambiguous
}

pub(crate) fn owner_pin_assertions(source: &str, trusted: &[&str]) -> OwnerPinAssertions {
    let mut result = OwnerPinAssertions::default();
    let Some(parse) = parse_clean_source_file(source) else {
        return result;
    };
    let lines = LineIndex::new(source);
    for module in parse
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Module::cast)
    {
        if module.item_list().is_some() {
            continue;
        }
        let (Some(token), Some(name)) = (module.mod_token(), module.name()) else {
            continue;
        };
        let key = (
            lines.line(token.text_range().start()),
            format!("mod {};", name.text()),
        );
        let admitted = supported_item_context(module.syntax());
        result
            .module_declarations
            .entry(key)
            .and_modify(|previous| *previous = false)
            .or_insert(admitted);
    }
    let mut identities = BTreeMap::<FunctionKey, usize>::new();
    for function in parse
        .tree()
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
    {
        let (Some(name), Some(token), Some(body)) =
            (function.name(), function.fn_token(), function.body())
        else {
            continue;
        };
        let key = (
            lines.line(token.text_range().start()),
            lines.line_for_range_end(function.syntax().text_range().end()),
            name.text().to_string(),
        );
        *identities.entry(key.clone()).or_default() += 1;
        if function.async_token().is_some()
            || has_escape(body.syntax(), trusted)
            || !supported_item_context(function.syntax())
            || function
                .attrs()
                .any(|attr| attr.simple_name().as_deref() != Some("test"))
        {
            continue;
        }
        let body_source = slice_text(
            source,
            token.text_range().start(),
            function.syntax().text_range().end(),
        );
        let mut candidates = BTreeMap::<AssertionKey, Vec<ast::MacroCall>>::new();
        for call in function
            .syntax()
            .descendants()
            .filter_map(ast::MacroCall::cast)
        {
            if call
                .path()
                .is_none_or(|path| path.syntax().text() != "assert_eq")
            {
                continue;
            }
            let range = call.syntax().text_range();
            let assertion = (
                lines.line(range.start()),
                slice_macro_call_text(source, range.start(), range.end()),
            );
            candidates.entry(assertion).or_default().push(call);
        }
        let macros = function
            .syntax()
            .descendants()
            .filter_map(ast::MacroCall::cast)
            .filter_map(|call| call.path())
            .map(|path| path.syntax().text().to_string())
            .collect();
        let assertions = candidates
            .into_iter()
            .filter_map(|(key, calls)| {
                // OracleFact has line/text, not an offset. No identical spelling
                // on the same line may borrow another invocation's context.
                (calls.len() == 1 && eager_path(calls[0].syntax().clone(), &function, false))
                    .then_some(key)
            })
            .collect();
        // Duplicate function identities are ambiguous too.
        if let std::collections::btree_map::Entry::Vacant(entry) =
            result.functions.entry(key.clone())
        {
            entry.insert(FunctionAssertions {
                body: body_source,
                assertions,
                macros,
            });
        } else {
            result.functions.insert(key, FunctionAssertions::default());
        }
    }
    result
        .functions
        .retain(|key, _| identities.get(key) == Some(&1));
    result
}

/// A libtest item cannot be nested in an executable body. Module/source
/// attributes include inner attributes on ItemList, not only outer attrs.
fn supported_item_context(item: &SyntaxNode) -> bool {
    let mut source_file = false;
    for (depth, node) in item.ancestors().enumerate() {
        if depth > 0
            && !ast::ItemList::can_cast(node.kind())
            && !ast::Module::can_cast(node.kind())
            && !ast::SourceFile::can_cast(node.kind())
        {
            return false;
        }
        if node.children().filter_map(ast::Attr::cast).any(|attr| {
            attribute_test_build_availability(&attr.syntax().text().to_string()) != Some(true)
        }) {
            return false;
        }
        source_file |= ast::SourceFile::can_cast(node.kind());
    }
    source_file
}

fn has_escape(body: &SyntaxNode, trusted: &[&str]) -> bool {
    body.descendants().any(|node| {
        if let Some(call) = ast::MacroCall::cast(node.clone()) {
            if call
                .path()
                .is_none_or(|path| !is_trusted_macro(&path.syntax().text().to_string(), trusted))
            {
                return true;
            }
            // Macro operands are opaque to AST descendant walks. Refuse
            // hidden exits and nested expansion, but not boolean negation.
            if call
                .token_tree()
                .is_some_and(|tree| opaque_macro_operand(&tree))
            {
                return true;
            }
        }

        ast::ReturnExpr::can_cast(node.kind())
            || (ast::TryExpr::can_cast(node.kind())
                && node
                    .ancestors()
                    .any(|parent| ast::ClosureExpr::can_cast(parent.kind())))
            || ast::BreakExpr::can_cast(node.kind())
            || ast::ContinueExpr::can_cast(node.kind())
            || ast::YieldExpr::can_cast(node.kind())
    })
}

fn opaque_macro_operand(tree: &ast::TokenTree) -> bool {
    let tokens: Vec<_> = tree
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| !token.kind().is_trivia())
        .collect();
    tokens.iter().any(|token| {
        matches!(
            token.text(),
            "return" | "break" | "continue" | "yield" | "?"
        )
    }) || tokens.windows(3).any(|tokens| {
        tokens[0].kind() == ra_ap_syntax::SyntaxKind::IDENT
            && tokens[1].text() == "!"
            && matches!(tokens[2].text(), "(" | "[" | "{")
    })
}

fn is_trusted_macro(path: &str, trusted: &[&str]) -> bool {
    // Qualified roots can themselves be rebound. Without name resolution,
    // only bare names with the workspace binding check are established.
    trusted.contains(&path)
}

fn eager_path(mut node: SyntaxNode, function: &ast::Fn, through_closure: bool) -> bool {
    loop {
        if node
            .children()
            .any(|child| ast::Attr::can_cast(child.kind()))
        {
            return false;
        }
        let Some(parent) = node.parent() else {
            return false;
        };
        if parent == *function.syntax() {
            return function.body().is_some_and(|body| body.syntax() == &node);
        }
        if let Some(closure) = ast::ClosureExpr::cast(parent.clone()) {
            if through_closure {
                return false;
            }
            let Some(call) = closure_invocation(&closure, function) else {
                return false;
            };
            return eager_path(call.syntax().clone(), function, true);
        }
        if let Some(block) = ast::BlockExpr::cast(parent.clone()) {
            if block.async_token().is_some()
                || block.const_token().is_some()
                || block.gen_token().is_some()
                || block.try_block_modifier().is_some()
                || block.label().is_some()
            {
                return false;
            }
        } else if let Some(binding) = ast::LetStmt::cast(parent.clone()) {
            if binding.let_else().is_some()
                || binding
                    .initializer()
                    .is_none_or(|expr| expr.syntax() != &node)
            {
                return false;
            }
        } else if !(ast::MacroExpr::can_cast(parent.kind())
            || ast::ExprStmt::can_cast(parent.kind())
            || ast::StmtList::can_cast(parent.kind())
            || ast::ParenExpr::can_cast(parent.kind()))
        {
            return false;
        }
        node = parent;
    }
}

fn closure_invocation(closure: &ast::ClosureExpr, function: &ast::Fn) -> Option<ast::CallExpr> {
    if closure.async_token().is_some()
        || closure.const_token().is_some()
        || closure.gen_token().is_some()
        || closure
            .param_list()
            .is_none_or(|params| params.params().next().is_some())
        || closure
            .syntax()
            .children()
            .any(|node| ast::Attr::can_cast(node.kind()))
    {
        return None;
    }
    let mut expression = closure.syntax().clone();
    while let Some(parent) = expression
        .parent()
        .filter(|node| ast::ParenExpr::can_cast(node.kind()))
    {
        expression = parent;
    }
    if let Some(call) = expression.parent().and_then(ast::CallExpr::cast) {
        return (call.expr().is_some_and(|expr| expr.syntax() == &expression)
            && no_arguments(&call))
        .then_some(call);
    }
    let binding = expression.parent().and_then(ast::LetStmt::cast)?;
    if binding
        .initializer()
        .is_none_or(|expr| expr.syntax() != &expression)
        || binding.let_else().is_some()
        || binding
            .syntax()
            .children()
            .any(|node| ast::Attr::can_cast(node.kind()))
    {
        return None;
    }
    if !eager_path(binding.syntax().clone(), function, true) {
        return None;
    }
    let scope = binding.syntax().parent()?;
    if !ast::StmtList::can_cast(scope.kind()) {
        return None;
    }
    let ast::Pat::IdentPat(pattern) = binding.pat()? else {
        return None;
    };
    if pattern.mut_token().is_some()
        || pattern.ref_token().is_some()
        || pattern.at_token().is_some()
    {
        return None;
    }
    let name = pattern.name()?.text().to_string();
    if name.starts_with("r#") {
        return None;
    }
    // Count tokens, including token trees, not only call expressions: aliases,
    // shadowing, macro arguments, mutation and capture are all unestablished.
    if function
        .syntax()
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.text().trim_start_matches("r#") == name)
        .count()
        != 2
    {
        return None;
    }
    function
        .syntax()
        .descendants()
        .filter_map(ast::CallExpr::cast)
        .find(|call| {
            call.syntax().text_range().start() >= binding.syntax().text_range().end()
                && call
                    .syntax()
                    .ancestors()
                    .skip(1)
                    .find(|node| ast::StmtList::can_cast(node.kind()))
                    .as_ref()
                    == Some(&scope)
                && no_arguments(call)
                && call.expr().is_some_and(|expr| {
                    matches!(expr, ast::Expr::PathExpr(_))
                        && expr.syntax().text().to_string() == name
                })
        })
}

fn no_arguments(call: &ast::CallExpr) -> bool {
    call.arg_list()
        .is_some_and(|args| args.args().next().is_none())
}
