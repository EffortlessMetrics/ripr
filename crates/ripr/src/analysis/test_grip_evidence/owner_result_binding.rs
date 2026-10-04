//! Parser-backed owner-result binding for FieldConstruction missing facts.
//!
//! A compatible `RequiredDiscriminator::FieldValue` fact is emitted only
//! when activation is already known and a unique parser-backed test binds
//! a plain local to a *direct* captured owner call, then weakly observes
//! that same binding's constructed field. Presence of a nearby test or
//! field name never manufactures activation. A grouped nested-`super`
//! import counts as that owner only when the resolved module uniquely
//! matches this seam's owner; `super::` itself is not a whitelist, and a
//! leading `::` extern-prelude path is not a local module.

use super::record_field_name;
use super::related_tests::call_text_contains_named_call;
use super::related_tests::context::{CompactGripContext, CompactTest};
use super::related_tests::module_path_for_index;
use crate::analysis::rust_index::{FunctionSummary, RustIndex};
use crate::analysis::seams::{RepoSeam, RequiredDiscriminator, SeamKind};
use crate::analysis::syntax::parse_clean_source_file;
use crate::analysis::syntax::ra::LineIndex;
use crate::domain::{MissingDiscriminatorFact, OracleKind, OracleStrength, StageState};
use ra_ap_syntax::ast::{self, HasName};
use ra_ap_syntax::{AstNode, SourceFile, SyntaxNode};
use std::path::Path;

pub(super) fn missing_field_value_facts(
    seam: &RepoSeam,
    related: &[&CompactTest<'_>],
    context: &CompactGripContext<'_>,
    owner_fn: Option<&FunctionSummary>,
    activation: &StageState,
) -> Vec<MissingDiscriminatorFact> {
    if *activation != StageState::Yes || seam.kind() != SeamKind::FieldConstruction {
        return Vec::new();
    }
    let Some(owner_fn) = owner_fn else {
        return Vec::new();
    };
    let owner_name = owner_fn.name.as_str();
    if owner_name.is_empty() {
        return Vec::new();
    }
    let owner_module = module_path_for_index(context.index, &owner_fn.file);
    let RequiredDiscriminator::FieldValue { field } = seam.required_discriminator() else {
        return Vec::new();
    };
    let Some(field_name) = record_field_name(field) else {
        return Vec::new();
    };

    let mut saw_strong = false;
    let mut saw_weak = false;
    for indexed in related {
        match owner_result_field_observation(
            indexed,
            context,
            owner_name,
            owner_module.as_deref(),
            field_name,
        ) {
            Some(OwnerResultObservation::Strong) => saw_strong = true,
            Some(OwnerResultObservation::Weak) => saw_weak = true,
            None => {}
        }
        if saw_strong {
            break;
        }
    }
    if saw_strong || !saw_weak {
        return Vec::new();
    }

    vec![MissingDiscriminatorFact {
        value: field.clone(),
        reason: format!(
            "owner-result field `{field_name}` is observed only by a weak oracle; an exact field-value discriminator is missing"
        ),
        flow_sink: None,
    }]
}

enum OwnerResultObservation {
    Strong,
    Weak,
}

fn owner_result_field_observation(
    indexed: &CompactTest<'_>,
    context: &CompactGripContext<'_>,
    owner_name: &str,
    owner_module: Option<&str>,
    field_name: &str,
) -> Option<OwnerResultObservation> {
    if !indexed.test.calls.iter().any(|call| {
        call.name == owner_name && call_text_contains_named_call(&call.text, owner_name)
    }) {
        return None;
    }
    let facts = context.index.files().get(indexed.test.file.as_path())?;
    if facts.used_lexical_fallback {
        return None;
    }
    let _ = context.unique_evidence_function(
        &indexed.test.file,
        &indexed.test.name,
        indexed.test.start_line,
    )?;
    let parse = context.parsed_source(&indexed.test.file)?;
    let lines = LineIndex::new(&facts.source);
    let function = unique_test_fn(
        &parse.tree(),
        &indexed.test.name,
        indexed.test.start_line,
        &lines,
    )?;
    if owner_callee_is_ambiguous(
        &function,
        owner_name,
        &indexed.test.file,
        owner_module,
        context.index,
    ) {
        return None;
    }
    let bindings = direct_owner_result_bindings(&function, owner_name, &lines);
    if bindings.is_empty() {
        return None;
    }
    let mut kind = None;
    for oracle in &indexed.test.assertions {
        for binding in &bindings {
            if oracle.line <= binding.line {
                continue;
            }
            if binding_invalidated_before(&function, binding, oracle.line, &lines) {
                continue;
            }
            if !oracle_observes_binding_field(&oracle.text, &binding.name, field_name) {
                continue;
            }
            match observation_from_oracle(oracle.kind.clone(), oracle.strength.clone()) {
                Some(OwnerResultObservation::Strong) => {
                    return Some(OwnerResultObservation::Strong);
                }
                Some(OwnerResultObservation::Weak) => kind = Some(OwnerResultObservation::Weak),
                None => {}
            }
        }
    }
    kind
}

struct OwnerResultBinding {
    name: String,
    line: usize,
    range_end: ra_ap_syntax::TextSize,
}

fn unique_test_fn(
    source: &SourceFile,
    name: &str,
    start_line: usize,
    lines: &LineIndex,
) -> Option<ast::Fn> {
    let mut matches = source
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .filter(|function| {
            function
                .name()
                .is_some_and(|fn_name| fn_name.text() == name)
                && function
                    .fn_token()
                    .is_some_and(|token| lines.line(token.text_range().start()) == start_line)
        });
    let function = matches.next()?;
    matches.next().is_none().then_some(function)
}

fn owner_callee_is_ambiguous(
    function: &ast::Fn,
    owner_name: &str,
    test_file: &Path,
    owner_module: Option<&str>,
    index: &RustIndex,
) -> bool {
    nested_owner_fn(function, owner_name)
        || sibling_owner_fn(function, owner_name)
        || foreign_owner_import(function, owner_name, test_file, owner_module, index)
        || local_owner_binding(function, owner_name)
}

fn local_owner_binding(function: &ast::Fn, owner_name: &str) -> bool {
    function
        .syntax()
        .descendants()
        .filter_map(ast::IdentPat::cast)
        .any(|pattern| {
            pattern.name().is_some_and(|name| name.text() == owner_name)
                && !ident_pat_is_direct_owner_result(&pattern, owner_name)
        })
}

fn ident_pat_is_direct_owner_result(pattern: &ast::IdentPat, owner_name: &str) -> bool {
    pattern
        .syntax()
        .parent()
        .and_then(ast::LetStmt::cast)
        .is_some_and(|binding| {
            plain_binding_name(&binding).as_deref() == Some(owner_name)
                && binding.initializer().is_some_and(|initializer| {
                    initializer_is_direct_owner_call(&initializer, owner_name)
                })
        })
}

fn nested_owner_fn(function: &ast::Fn, owner_name: &str) -> bool {
    function
        .syntax()
        .descendants()
        .filter_map(ast::Fn::cast)
        .any(|nested| {
            nested.syntax().text_range() != function.syntax().text_range()
                && nested.name().is_some_and(|name| name.text() == owner_name)
        })
}

fn sibling_owner_fn(function: &ast::Fn, owner_name: &str) -> bool {
    let Some(container) = enclosing_item_container(function) else {
        return false;
    };
    if ast::SourceFile::can_cast(container.kind()) {
        return false;
    }
    item_children(&container)
        .into_iter()
        .filter_map(ast::Fn::cast)
        .any(|sibling| {
            sibling.syntax().text_range() != function.syntax().text_range()
                && sibling.name().is_some_and(|name| name.text() == owner_name)
        })
}

fn foreign_owner_import(
    function: &ast::Fn,
    owner_name: &str,
    test_file: &Path,
    owner_module: Option<&str>,
    index: &RustIndex,
) -> bool {
    let resolved = owner_module.and_then(|owner_module| {
        module_path_for_index(index, test_file).map(|file_module| (owner_module, file_module))
    });
    owner_scope_uses(function).any(|item| match &resolved {
        Some((owner_module, file_module)) => {
            let current_module = use_current_module(&item, file_module);
            use_binds_foreign_owner(&item, owner_name, &current_module, owner_module, index)
        }
        None => use_tree_binds_owner_name(item.use_tree().as_ref(), owner_name),
    })
}

fn owner_scope_uses(function: &ast::Fn) -> impl Iterator<Item = ast::Use> {
    let container_uses = enclosing_item_container(function)
        .into_iter()
        .flat_map(|container| {
            item_children(&container)
                .into_iter()
                .filter_map(ast::Use::cast)
        });
    let nested_uses = function.syntax().descendants().filter_map(ast::Use::cast);
    container_uses.chain(nested_uses)
}

fn use_tree_binds_owner_name(tree: Option<&ast::UseTree>, owner_name: &str) -> bool {
    let Some(tree) = tree else {
        return false;
    };
    if tree.star_token().is_some() {
        return false;
    }
    if let Some(list) = tree.use_tree_list() {
        return list
            .use_trees()
            .any(|nested| use_tree_binds_owner_name(Some(&nested), owner_name));
    }
    use_tree_local_name(tree).as_deref() == Some(owner_name)
}

fn use_tree_path_text(tree: &ast::UseTree) -> Option<String> {
    tree.path().map(|path| {
        let mut text: String = path
            .syntax()
            .text()
            .to_string()
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let leading_extern = path
            .syntax()
            .first_child_or_token()
            .is_some_and(|element| element.kind() == ra_ap_syntax::SyntaxKind::COLON2);
        if leading_extern && !text.starts_with("::") {
            text.insert_str(0, "::");
        }
        text
    })
}

fn use_tree_local_name(tree: &ast::UseTree) -> Option<String> {
    tree.rename()
        .and_then(|rename| rename.name())
        .map(|name| name.text().to_string())
        .or_else(|| {
            use_tree_path_text(tree)
                .as_ref()
                .and_then(|path| path.rsplit("::").next())
                .map(ToString::to_string)
        })
}

fn enclosing_item_container(function: &ast::Fn) -> Option<SyntaxNode> {
    function
        .syntax()
        .ancestors()
        .find(|node| ast::Module::can_cast(node.kind()) || ast::SourceFile::can_cast(node.kind()))
}

fn item_children(container: &SyntaxNode) -> Vec<SyntaxNode> {
    ast::Module::cast(container.clone())
        .and_then(|module| module.item_list())
        .map(|list| list.syntax().children().collect())
        .unwrap_or_else(|| container.children().collect())
}

fn use_binds_foreign_owner(
    item: &ast::Use,
    owner_name: &str,
    current_module: &str,
    owner_module: &str,
    index: &RustIndex,
) -> bool {
    item.use_tree().is_some_and(|tree| {
        use_tree_binds_foreign_owner(&tree, "", owner_name, current_module, owner_module, index)
    })
}

fn use_current_module(item: &ast::Use, file_module: &str) -> String {
    let nested: Vec<String> = item
        .syntax()
        .ancestors()
        .filter_map(ast::Module::cast)
        .filter_map(|module| module.name().map(|name| name.text().to_string()))
        .collect();
    let mut segments: Vec<String> = file_module
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect();
    for name in nested.into_iter().rev() {
        segments.push(name);
    }
    segments.join("/")
}

fn use_tree_binds_foreign_owner(
    tree: &ast::UseTree,
    prefix: &str,
    owner_name: &str,
    current_module: &str,
    owner_module: &str,
    index: &RustIndex,
) -> bool {
    if tree.star_token().is_some() {
        return false;
    }
    let path_text = use_tree_path_text(tree);
    if let Some(list) = tree.use_tree_list() {
        let child_prefix = match (&path_text, prefix.is_empty()) {
            (Some(path), false) => format!("{prefix}::{path}"),
            (Some(path), true) => path.clone(),
            (None, _) => prefix.to_string(),
        };
        return list.use_trees().any(|nested| {
            use_tree_binds_foreign_owner(
                &nested,
                &child_prefix,
                owner_name,
                current_module,
                owner_module,
                index,
            )
        });
    }
    if use_tree_local_name(tree).as_deref() != Some(owner_name) {
        return false;
    }
    let full = match (&path_text, prefix.is_empty()) {
        (Some(path), false) => format!("{prefix}::{path}"),
        (Some(path), true) => path.clone(),
        (None, false) => prefix.to_string(),
        (None, true) => owner_name.to_string(),
    };
    !import_resolves_to_unique_owner(&full, current_module, owner_module, owner_name, index)
}

fn import_resolves_to_unique_owner(
    item_path: &str,
    current_module: &str,
    owner_module: &str,
    owner_name: &str,
    index: &RustIndex,
) -> bool {
    let Some(resolved_module) = resolve_use_module_path(item_path, current_module) else {
        return false;
    };
    if normalize_module_key(&resolved_module) != normalize_module_key(owner_module) {
        return false;
    }
    owner_name_is_unique_in_module(index, owner_module, owner_name)
}

fn resolve_use_module_path(item_path: &str, current_module: &str) -> Option<String> {
    // Rust 2018+ `::ident` selects the extern prelude, not a local module.
    if item_path.starts_with("::") {
        return None;
    }
    let mut segments: Vec<String> = current_module
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect();
    let parts: Vec<&str> = item_path
        .split("::")
        .filter(|part| !part.is_empty())
        .collect();
    if parts.is_empty() {
        return None;
    }
    for part in &parts[..parts.len() - 1] {
        match *part {
            "super" => {
                segments.pop()?;
            }
            "self" => {}
            "crate" => segments.clear(),
            other => segments.push(other.to_string()),
        }
    }
    Some(segments.join("/"))
}

fn owner_name_is_unique_in_module(index: &RustIndex, module_path: &str, owner_name: &str) -> bool {
    let normalized = normalize_module_key(module_path);
    let mut seen = false;
    for function in &index.functions() {
        if function.source_role.is_evidence_role() {
            continue;
        }
        if function.name != owner_name {
            continue;
        }
        let Some(path) = module_path_for_index(index, &function.file) else {
            continue;
        };
        if normalize_module_key(&path) != normalized {
            continue;
        }
        if seen {
            return false;
        }
        seen = true;
    }
    seen
}

fn normalize_module_key(path: &str) -> String {
    path.replace('/', "::")
}

fn direct_owner_result_bindings(
    function: &ast::Fn,
    owner_name: &str,
    lines: &LineIndex,
) -> Vec<OwnerResultBinding> {
    let Some(statements) = function.body().and_then(|body| body.stmt_list()) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for statement in statements.statements() {
        let ast::Stmt::LetStmt(binding) = statement else {
            continue;
        };
        let Some(name) = plain_binding_name(&binding) else {
            continue;
        };
        let Some(initializer) = binding.initializer() else {
            continue;
        };
        if !initializer_is_direct_owner_call(&initializer, owner_name) {
            continue;
        }
        let line = binding
            .let_token()
            .map(|token| lines.line(token.text_range().start()))
            .unwrap_or_else(|| lines.line(binding.syntax().text_range().start()));
        found.push(OwnerResultBinding {
            name,
            line,
            range_end: binding.syntax().text_range().end(),
        });
    }
    found
}

fn plain_binding_name(statement: &ast::LetStmt) -> Option<String> {
    let pattern = ast::IdentPat::cast(statement.pat()?.syntax().clone())?;
    if pattern.ref_token().is_some() || pattern.at_token().is_some() || pattern.pat().is_some() {
        return None;
    }
    let name = pattern.name()?.text().to_string();
    valid_identifier(&name).then_some(name)
}

fn initializer_is_direct_owner_call(expression: &ast::Expr, owner_name: &str) -> bool {
    let Some(call) = ast::CallExpr::cast(expression.syntax().clone()) else {
        return false;
    };
    let Some(callee) = call.expr() else {
        return false;
    };
    direct_name(&callee).as_deref() == Some(owner_name)
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

fn binding_invalidated_before(
    function: &ast::Fn,
    binding: &OwnerResultBinding,
    assertion_line: usize,
    lines: &LineIndex,
) -> bool {
    let assertion_cutoff = function
        .syntax()
        .descendants()
        .find(|node| lines.line(node.text_range().start()) == assertion_line)
        .map(|node| node.text_range().start())
        .unwrap_or_else(|| function.syntax().text_range().end());
    let start = binding.range_end;
    if start >= assertion_cutoff {
        return true;
    }
    for node in function.syntax().descendants() {
        let range = node.text_range();
        if range.start() < start || range.start() >= assertion_cutoff {
            continue;
        }
        if let Some(pattern) = ast::IdentPat::cast(node.clone())
            && pattern
                .name()
                .is_some_and(|name| name.text() == binding.name)
        {
            return true;
        }
        if let Some(bin) = ast::BinExpr::cast(node.clone())
            && is_assignment_op(
                bin.op_token()
                    .as_ref()
                    .map(|token| token.text())
                    .unwrap_or(""),
            )
            && let Some(lhs) = bin.lhs()
            && binding_escape(&lhs, &binding.name)
        {
            return true;
        }
        if let Some(reference) = ast::RefExpr::cast(node)
            && reference.mut_token().is_some()
            && reference
                .expr()
                .is_some_and(|expr| binding_escape(&expr, &binding.name))
        {
            return true;
        }
    }
    false
}

fn is_assignment_op(token: &str) -> bool {
    matches!(
        token,
        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>="
    )
}

fn binding_escape(expression: &ast::Expr, binding: &str) -> bool {
    if path_is_binding(expression, binding) || field_receiver_is_binding(expression, binding) {
        return true;
    }
    ast::ParenExpr::cast(expression.syntax().clone())
        .and_then(|paren| paren.expr())
        .is_some_and(|inner| binding_escape(&inner, binding))
}

fn path_is_binding(expression: &ast::Expr, binding: &str) -> bool {
    direct_name(expression).as_deref() == Some(binding)
}

fn field_receiver_is_binding(expression: &ast::Expr, binding: &str) -> bool {
    ast::FieldExpr::cast(expression.syntax().clone())
        .and_then(|field| field.expr())
        .is_some_and(|receiver| path_is_binding(&receiver, binding))
}

fn oracle_observes_binding_field(oracle_text: &str, binding: &str, field: &str) -> bool {
    let Some(source) = discriminating_oracle_source(oracle_text) else {
        return false;
    };
    let wrapped = format!("fn __ripr_owner_result_probe() {{ {source}\n}}");
    let Some(parse) = parse_clean_source_file(&wrapped) else {
        return false;
    };
    probe_reads_binding_field(&parse.tree(), binding, field)
}

fn discriminating_oracle_source(oracle_text: &str) -> Option<String> {
    let text = oracle_text.trim().trim_end_matches(';').trim();
    let (macro_name, open) = assertion_macro_open(text)?;
    let inner = super::delimited_contents_at(text, open)?;
    let mut args = super::split_top_level_commas(&inner).into_iter();
    match macro_name {
        "assert" => args.next().filter(|arg| !arg.is_empty()),
        "assert_eq" | "assert_ne" => {
            let left = args.next().filter(|arg| !arg.is_empty())?;
            let right = args.next().filter(|arg| !arg.is_empty())?;
            Some(format!("({left}, {right})"))
        }
        _ => None,
    }
}

fn assertion_macro_open(text: &str) -> Option<(&'static str, usize)> {
    ["assert_eq!", "assert_ne!", "assert!"]
        .into_iter()
        .find_map(|macro_name| {
            text.match_indices(macro_name).find_map(|(index, _)| {
                let prefix_ok = index == 0
                    || !text[..index]
                        .chars()
                        .next_back()
                        .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_');
                let suffix_start = index + macro_name.len();
                let open_offset = text[suffix_start..]
                    .char_indices()
                    .find_map(|(offset, ch)| (!ch.is_whitespace()).then_some((offset, ch)))?;
                if !prefix_ok || open_offset.1 != '(' {
                    return None;
                }
                let name = macro_name.trim_end_matches('!');
                Some((name, suffix_start + open_offset.0))
            })
        })
}

fn probe_reads_binding_field(probe: &SourceFile, binding: &str, field: &str) -> bool {
    probe
        .syntax()
        .descendants()
        .filter_map(ast::FieldExpr::cast)
        .any(|expr| {
            expr.name_ref().is_some_and(|name| name.text() == field)
                && expr
                    .expr()
                    .is_some_and(|receiver| path_is_binding(&receiver, binding))
                && !ident_pat_shadows_before(probe, binding, expr.syntax().text_range().start())
        })
}

fn ident_pat_shadows_before(probe: &SourceFile, binding: &str, at: ra_ap_syntax::TextSize) -> bool {
    probe
        .syntax()
        .descendants()
        .filter_map(ast::IdentPat::cast)
        .any(|pattern| {
            pattern.name().is_some_and(|name| name.text() == binding)
                && pattern.syntax().text_range().start() < at
        })
}

fn observation_from_oracle(
    kind: OracleKind,
    strength: OracleStrength,
) -> Option<OwnerResultObservation> {
    match (kind, strength) {
        (
            OracleKind::ExactValue | OracleKind::WholeObjectEquality,
            OracleStrength::Strong | OracleStrength::Medium,
        ) => Some(OwnerResultObservation::Strong),
        (OracleKind::RelationalCheck, OracleStrength::Strong | OracleStrength::Medium) => {
            Some(OwnerResultObservation::Strong)
        }
        (
            OracleKind::Snapshot | OracleKind::SmokeOnly,
            OracleStrength::Medium | OracleStrength::Strong,
        ) => Some(OwnerResultObservation::Strong),
        (
            OracleKind::RelationalCheck | OracleKind::Snapshot | OracleKind::SmokeOnly,
            OracleStrength::Weak | OracleStrength::Smoke,
        ) => Some(OwnerResultObservation::Weak),
        _ => None,
    }
}
