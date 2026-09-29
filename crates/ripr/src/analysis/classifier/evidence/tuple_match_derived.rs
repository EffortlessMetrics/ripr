//! Bounded discrimination for two derived Boolean locals inside one closure.
//!
//! This extends the direct-parameter tuple witness without becoming a Rust
//! interpreter. Every unsupported owner, closure, initializer, result-flow, or
//! oracle shape leaves the existing weak evidence unchanged.

use crate::analysis::classify::ProbeContext;
use crate::analysis::rust_index::find_file_facts;
use crate::analysis::syntax::parse_clean_source_file;
use crate::domain::{Confidence, Probe, ProbeFamily, RelationReason, StageEvidence, StageState};
use ra_ap_syntax::ast::{HasArgList, HasAttrs, HasName};
use ra_ap_syntax::{AstNode, SyntaxNode, ast};
use std::path::{Component, Path, PathBuf};

struct DerivedWitness {
    input: [bool; 2],
    result: String,
}

/// Refine only a still-missing discriminator for the bounded derived-local
/// closure shape. Reach, infection, propagation, observation, and final class
/// remain owned by the ordinary stage combiner.
pub(super) fn discrimination(
    context: &ProbeContext<'_>,
    observe: &StageEvidence,
    current: &StageEvidence,
) -> Option<StageEvidence> {
    if context.probe.family != ProbeFamily::MatchArm
        || observe.state != StageState::Yes
        || current.state != StageState::Weak
        || !current.summary.contains("observation_unverified")
        || !context.workspace_complete
    {
        return None;
    }

    let owner = context.owner_fn?;
    if context.probe.owner.as_ref() != Some(&owner.id)
        || context
            .index
            .functions
            .iter()
            .filter(|function| function.name == owner.name)
            .count()
            != 1
    {
        return None;
    }

    let facts = find_file_facts(context.index, &owner.file)?;
    let same_source = find_file_facts(context.index, &context.probe.location.file)
        .is_some_and(|probe_facts| std::ptr::eq(facts, probe_facts))
        || same_current_file(
            context,
            &owner.file,
            &context.probe.location.file,
            &facts.source,
        );
    if !same_source || facts.used_lexical_fallback {
        return None;
    }

    let root = parsed(&facts.source)?;
    let function = named_function(&root, &owner.name)?;
    let witness = derived_witness(&facts.source, &function, context.probe)?;

    for (test, reason) in &context.related_tests {
        if *reason != RelationReason::DirectOwnerCall {
            continue;
        }
        let Some(test_facts) = find_file_facts(context.index, &test.file) else {
            continue;
        };
        if test_facts.used_lexical_fallback
            || !test_facts.role_provenance.edges.is_empty()
            || test_facts
                .role_provenance
                .earliest_unresolved_reason
                .is_some()
            || !assertion_namespace_is_standalone(&owner.file, &test.file)
            || context.test_file_imports_foreign_callee_name(
                &test.file,
                &test_facts.source,
                &owner.name,
            )
        {
            continue;
        }
        let Some(test_root) = parsed(&test_facts.source) else {
            continue;
        };
        if !test_namespace_is_plain(&test_root) {
            continue;
        }
        let Some(test_function) = named_function(&test_root, &test.name) else {
            continue;
        };
        // `derived_admission` owns the projection oracle. Re-check its one
        // strict implementation here so this path keeps the same guarantees
        // even if the admission wiring in `tuple_match::discrimination` ever
        // changes; the former local weaker copy is deleted.
        if super::derived_admission::top_level_projection_observes(
            &test_function,
            &owner.name,
            witness.result.as_str(),
        ) == Some(true)
        {
            return Some(StageEvidence::new(
                StageState::Yes,
                Confidence::High,
                "Exact derived Boolean tuple selects this current arm and the returned projection observes its unique literal result",
            ));
        }
    }

    None
}

fn derived_witness(source: &str, function: &ast::Fn, probe: &Probe) -> Option<DerivedWitness> {
    if function.attrs().next().is_some() || function.async_token().is_some() {
        return None;
    }
    let body = function.body()?.stmt_list()?;
    if body.statements().next().is_some() {
        return None;
    }
    let tail = body.tail_expr()?;
    let tail_text = compact(&tail.syntax().text().to_string());
    if !tail_text.contains(".iter().filter_map(") || !tail_text.ends_with(".collect()") {
        return None;
    }

    let mut selected = None;
    for match_expression in function
        .syntax()
        .descendants()
        .filter_map(ast::MatchExpr::cast)
    {
        let Some(candidate) = derived_match_witness(source, &match_expression, probe) else {
            continue;
        };
        if selected.is_some() {
            return None;
        }
        selected = Some(candidate);
    }
    selected
}

fn derived_match_witness(
    source: &str,
    match_expression: &ast::MatchExpr,
    probe: &Probe,
) -> Option<DerivedWitness> {
    let tuple = ast::TupleExpr::cast(match_expression.expr()?.syntax().clone())?;
    let tuple_fields = tuple
        .syntax()
        .children()
        .filter_map(ast::Expr::cast)
        .collect::<Vec<_>>();
    let [left, right] = tuple_fields.as_slice() else {
        return None;
    };
    let tuple_names = [direct_name(left)?, direct_name(right)?];
    if tuple_names[0] == tuple_names[1] {
        return None;
    }

    let relation_let = match_expression
        .syntax()
        .ancestors()
        .find_map(ast::LetStmt::cast)?;
    let initializer = relation_let.initializer()?;
    if initializer.syntax().text_range() != match_expression.syntax().text_range() {
        return None;
    }
    let relation_name = immutable_binding_name(&relation_let)?;
    if tuple_names.contains(&relation_name) {
        return None;
    }

    let closure = match_expression
        .syntax()
        .ancestors()
        .find_map(ast::ClosureExpr::cast)?;
    let closure_parameter = closure_parameter(&closure)?;
    let closure_body = ast::BlockExpr::cast(closure.body()?.syntax().clone())?;
    let statements = closure_body.stmt_list()?.statements().collect::<Vec<_>>();
    let [
        ast::Stmt::LetStmt(first),
        ast::Stmt::LetStmt(second),
        ast::Stmt::LetStmt(relation),
    ] = statements.as_slice()
    else {
        return None;
    };
    if relation.syntax().text_range() != relation_let.syntax().text_range()
        || immutable_binding_name(first)? != tuple_names[0]
        || immutable_binding_name(second)? != tuple_names[1]
        || immutable_binding_name(relation)? != relation_name
    {
        return None;
    }

    let first_initializer = first.initializer()?;
    let second_initializer = second.initializer()?;
    if !supported_request_membership(&first_initializer, &closure_parameter)
        || !supported_task_identity(&second_initializer, &closure_parameter)
    {
        return None;
    }

    let closure_tail = closure_body.stmt_list()?.tail_expr()?;
    if !returns_relation(&closure_tail, &closure_parameter, &relation_name) {
        return None;
    }

    current_arm(source, match_expression, probe)
}

fn current_arm(
    source: &str,
    match_expression: &ast::MatchExpr,
    probe: &Probe,
) -> Option<DerivedWitness> {
    let after = probe.after.as_deref()?;
    let arms = match_expression
        .match_arm_list()?
        .arms()
        .collect::<Vec<_>>();
    if arms.len() != 4 {
        return None;
    }

    let mut seen = Vec::new();
    let mut literal_results = Vec::new();
    let mut selected = None;
    for arm in arms {
        if arm.guard().is_some() || arm.attrs().next().is_some() {
            return None;
        }
        let input = bool_pattern(&arm.pat()?)?;
        if seen.contains(&input) {
            return None;
        }
        seen.push(input);

        if input == [false, false] {
            if !returns_none(&arm.expr()?) {
                return None;
            }
            continue;
        }

        let result = plain_string(&arm.expr()?)?;
        if literal_results.contains(&result) {
            return None;
        }
        literal_results.push(result.clone());

        let start = u32::from(arm.syntax().text_range().start()) as usize;
        let line = source
            .get(..start)?
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1;
        if line == probe.location.line
            && arm_source_matches(&arm, after)?
            && arm_source_matches(&arm, &probe.expression)?
        {
            if selected.is_some() {
                return None;
            }
            selected = Some(DerivedWitness { input, result });
        }
    }

    if ![[true, true], [true, false], [false, true], [false, false]]
        .iter()
        .all(|input| seen.contains(input))
    {
        return None;
    }

    let witness = selected?;
    if let Some(before) = probe.before.as_deref() {
        let old = single_arm(before)?;
        if old.guard().is_some()
            || old.attrs().next().is_some()
            || bool_pattern(&old.pat()?)? != witness.input
            || plain_string(&old.expr()?)? == witness.result
        {
            return None;
        }
    }
    Some(witness)
}

fn supported_request_membership(expression: &ast::Expr, closure_parameter: &str) -> bool {
    let text = compact(&expression.syntax().text().to_string());
    let receipt_lookup = format!(".get(&{closure_parameter}.id).is_some_and(");
    text.contains(&receipt_lookup)
        && text.contains(".iter().any(")
        && text.contains(".contains(")
        && !text.contains("!=")
        && !text.starts_with('!')
}

fn supported_task_identity(expression: &ast::Expr, closure_parameter: &str) -> bool {
    let text = compact(&expression.syntax().text().to_string());
    if text.contains("!=") || text.matches("==").count() != 1 {
        return false;
    }
    let Some((left, right)) = text.split_once("==") else {
        return false;
    };
    let receipt_id = format!("{closure_parameter}.id");
    (left == receipt_id && valid_identifier(right))
        || (right == receipt_id && valid_identifier(left))
}

fn returns_relation(expression: &ast::Expr, closure_parameter: &str, relation: &str) -> bool {
    let Some(call) = ast::CallExpr::cast(expression.syntax().clone()) else {
        return false;
    };
    let Some(callee) = call.expr() else {
        return false;
    };
    if direct_name(&callee).as_deref() != Some("Some") {
        return false;
    }
    let Some(argument_list) = call.arg_list() else {
        return false;
    };
    let arguments = argument_list.args().collect::<Vec<_>>();
    let [argument] = arguments.as_slice() else {
        return false;
    };
    let Some(tuple) = ast::TupleExpr::cast(argument.syntax().clone()) else {
        return false;
    };
    let fields = tuple
        .syntax()
        .children()
        .filter_map(ast::Expr::cast)
        .collect::<Vec<_>>();
    let [receipt, result] = fields.as_slice() else {
        return false;
    };
    direct_name(receipt).as_deref() == Some(closure_parameter)
        && direct_name(result).as_deref() == Some(relation)
}

fn returns_none(expression: &ast::Expr) -> bool {
    let Some(return_expression) = ast::ReturnExpr::cast(expression.syntax().clone()) else {
        return false;
    };
    return_expression
        .expr()
        .and_then(|value| direct_name(&value))
        .as_deref()
        == Some("None")
}

fn closure_parameter(closure: &ast::ClosureExpr) -> Option<String> {
    let text = closure.syntax().text().to_string();
    let rest = text.strip_prefix('|')?;
    let end = rest.find('|')?;
    let parameter = rest.get(..end)?.trim();
    valid_identifier(parameter).then(|| parameter.to_string())
}

fn immutable_binding_name(statement: &ast::LetStmt) -> Option<String> {
    let pattern = ast::IdentPat::cast(statement.pat()?.syntax().clone())?;
    let name = pattern.name()?.text().to_string();
    (pattern.syntax().text().to_string().trim() == name).then_some(name)
}

fn valid_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn compact(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn direct_name(expression: &ast::Expr) -> Option<String> {
    let path = ast::PathExpr::cast(expression.syntax().clone())?.path()?;
    let text = path.syntax().text().to_string();
    valid_identifier(&text).then_some(text)
}

fn bool_pattern(pattern: &ast::Pat) -> Option<[bool; 2]> {
    let tuple = ast::TuplePat::cast(pattern.syntax().clone())?;
    let fields = tuple
        .syntax()
        .children()
        .filter_map(ast::Pat::cast)
        .collect::<Vec<_>>();
    let [first, second] = fields.as_slice() else {
        return None;
    };
    Some([boolean(first.syntax())?, boolean(second.syntax())?])
}

fn boolean(node: &SyntaxNode) -> Option<bool> {
    match node.text().to_string().trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn plain_string(expression: &ast::Expr) -> Option<String> {
    let literal = ast::Literal::cast(expression.syntax().clone())?;
    let text = literal.syntax().text().to_string();
    let value = text.strip_prefix('"')?.strip_suffix('"')?;
    (!value.contains(['\\', '"', '\n', '\r'])).then(|| value.to_string())
}

fn single_arm(text: &str) -> Option<ast::MatchArm> {
    let source = format!("fn __arm() {{ match (false, false) {{ {text} }} }}");
    let root = parsed(&source)?;
    let function = named_function(&root, "__arm")?;
    let body = function.body()?.stmt_list()?;
    if body.statements().next().is_some() {
        return None;
    }
    let expression = ast::MatchExpr::cast(body.tail_expr()?.syntax().clone())?;
    let list = expression.match_arm_list()?;
    let mut arms = list.arms();
    let arm = arms.next()?;
    arms.next().is_none().then_some(arm)
}

fn arm_text(text: &str) -> &str {
    text.trim().trim_end_matches(',').trim_end()
}

fn arm_source_matches(arm: &ast::MatchArm, claimed: &str) -> Option<bool> {
    let full = arm.syntax().text().to_string();
    let start = u32::from(arm.syntax().text_range().start());
    let end = u32::from(arm.fat_arrow_token()?.text_range().end());
    let boundary = end.checked_sub(start)? as usize;
    let pattern = full.get(..boundary)?;
    Some(arm_text(&full) == arm_text(claimed) || pattern.trim() == claimed.trim())
}

fn parsed(source: &str) -> Option<ast::SourceFile> {
    parse_clean_source_file(source).map(|parse| parse.tree())
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

fn test_namespace_is_plain(root: &ast::SourceFile) -> bool {
    root.syntax().children().all(|node| {
        if ast::Fn::can_cast(node.kind()) {
            return true;
        }
        if let Some(import) = ast::Use::cast(node) {
            let text = import.syntax().text().to_string();
            return !text.contains('*') && !text.contains("assert_eq") && !text.contains(" as ");
        }
        false
    })
}

fn same_current_file(
    context: &ProbeContext<'_>,
    owner_file: &Path,
    probe_file: &Path,
    source: &str,
) -> bool {
    let Some(authority) = context.index.workspace_authority.as_ref() else {
        return false;
    };
    if !authority.validates_target(owner_file, owner_file, source) {
        return false;
    }
    let Ok(owner_path) = authority.root.join(owner_file).canonicalize() else {
        return false;
    };
    // Probe locations are `--root` as given joined with the changed path, so
    // they resolve from the working directory. Joining them onto the
    // canonical authority root again only works for an absolute or `.` root.
    let Ok(probe_path) = probe_file.canonicalize() else {
        return false;
    };
    owner_path == probe_path
}

fn assertion_namespace_is_standalone(owner_file: &Path, test_file: &Path) -> bool {
    if test_file == owner_file {
        return conventional_source_crate_root(test_file);
    }
    let Some(package_root) = conventional_package_root(owner_file) else {
        return false;
    };
    let relative_test = if package_root.as_os_str().is_empty() {
        test_file
    } else {
        let Ok(relative) = test_file.strip_prefix(&package_root) else {
            return false;
        };
        relative
    };
    let mut components = relative_test.components();
    let (Some(Component::Normal(directory)), Some(Component::Normal(file)), None) =
        (components.next(), components.next(), components.next())
    else {
        return false;
    };
    directory == "tests"
        && Path::new(file)
            .extension()
            .is_some_and(|extension| extension == "rs")
}

fn conventional_source_crate_root(file: &Path) -> bool {
    let Some(package_root) = conventional_package_root(file) else {
        return false;
    };
    let relative = if package_root.as_os_str().is_empty() {
        file
    } else {
        let Ok(relative) = file.strip_prefix(&package_root) else {
            return false;
        };
        relative
    };
    let mut components = relative.components();
    match (
        components.next(),
        components.next(),
        components.next(),
        components.next(),
    ) {
        (Some(Component::Normal(source)), Some(Component::Normal(file)), None, None) => {
            source == "src" && matches!(file.to_str(), Some("lib.rs" | "main.rs"))
        }
        (
            Some(Component::Normal(source)),
            Some(Component::Normal(bin)),
            Some(Component::Normal(file)),
            None,
        ) => {
            source == "src"
                && bin == "bin"
                && Path::new(file)
                    .extension()
                    .is_some_and(|extension| extension == "rs")
        }
        _ => false,
    }
}

fn conventional_package_root(owner_file: &Path) -> Option<PathBuf> {
    let components = owner_file.components().collect::<Vec<_>>();
    let source_index = components
        .iter()
        .rposition(|component| matches!(component, Component::Normal(name) if *name == "src"))?;
    if source_index + 1 >= components.len() {
        return None;
    }
    let mut root = PathBuf::new();
    for component in &components[..source_index] {
        let Component::Normal(name) = component else {
            return None;
        };
        root.push(name);
    }
    Some(root)
}
