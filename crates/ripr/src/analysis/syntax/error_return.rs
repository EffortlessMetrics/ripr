use super::parse_clean_source_file;
use crate::analysis::facts::FunctionSummary;
use crate::domain::{Probe, ProbeFamily};
use ra_ap_syntax::{AstNode, ast, ast::HasArgList};

/// A bounded producer replacement whose before value is opaque. This does
/// not establish equivalence: it withholds infection credit until divergence
/// between the constructor call and inline value can be established.
pub(crate) fn guarded_opaque_error_transition(
    probe: &Probe,
    owner: &FunctionSummary,
) -> Option<(String, String)> {
    if !matches!(
        probe.family,
        ProbeFamily::ErrorPath | ProbeFamily::ReturnValue | ProbeFamily::FieldConstruction
    ) {
        return None;
    }
    let after = probe.after.as_deref()?.trim();
    if after.is_empty() {
        return None;
    }
    let before = probe.before.as_deref()?;
    let before_source = format!("fn before() {{ {before} }}");
    let before_parse = parse_clean_source_file(&before_source)?;
    let mut before_returns = before_parse
        .syntax_node()
        .descendants()
        .filter_map(ast::ReturnExpr::cast);
    let before_return = before_returns.next()?;
    if before_returns.next().is_some() {
        return None;
    }
    let before_function = before_parse
        .syntax_node()
        .children()
        .find_map(ast::Fn::cast)?;
    if !return_belongs_to_function(&before_return, &before_function) {
        return None;
    }
    let before_value = error_argument(&before_return)?;
    let before_call = ast::CallExpr::cast(before_value.syntax().clone())?;
    if before_call.arg_list()?.args().next().is_some() {
        return None;
    }
    let producer = before_call.expr()?.syntax().text().to_string();
    if ast::PathExpr::cast(before_call.expr()?.syntax().clone())?
        .path()?
        .qualifier()
        .is_some()
    {
        return None;
    }

    let parse = parse_clean_source_file(&owner.body)?;
    let function = parse.syntax_node().children().find_map(ast::Fn::cast)?;
    let line = |node: &ra_ap_syntax::SyntaxNode| {
        let offset = u32::from(node.text_range().start()) as usize;
        owner.start_line
            + owner.body[..offset]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
    };
    let mut returns = function
        .syntax()
        .descendants()
        .filter_map(ast::ReturnExpr::cast)
        .filter(|item| line(item.syntax()) == probe.location.line);
    let returned = returns.next()?;
    if returns.next().is_some()
        || !return_belongs_to_function(&returned, &function)
        || !returned.syntax().text().to_string().contains(after)
        || !matches!(error_argument(&returned)?, ast::Expr::RecordExpr(_))
    {
        return None;
    }
    for ancestor in returned.syntax().ancestors().skip(1) {
        if ancestor == *function.syntax() {
            break;
        }
        let Some(guarded) = ast::IfExpr::cast(ancestor) else {
            continue;
        };
        let then = guarded.then_branch()?;
        if !then
            .syntax()
            .text_range()
            .contains_range(returned.syntax().text_range())
        {
            return None;
        }
        let condition = ast::PathExpr::cast(guarded.condition()?.syntax().clone())?;
        let guard = condition.syntax().text().to_string();
        if condition.path()?.qualifier().is_some()
            || owner
                .let_bindings
                .iter()
                .any(|binding| binding.name == guard)
        {
            return None;
        }
        let parameter = function.param_list()?.params().any(|parameter| {
            parameter.pat().is_some_and(|pattern| {
                let name = pattern.syntax().text().to_string();
                name.strip_prefix("mut ").unwrap_or(&name) == guard
            }) && parameter
                .ty()
                .is_some_and(|ty| ty.syntax().text() == "bool")
        });
        return parameter.then_some((guard, producer));
    }
    None
}

fn return_belongs_to_function(returned: &ast::ReturnExpr, function: &ast::Fn) -> bool {
    returned
        .syntax()
        .ancestors()
        .skip(1)
        .find(|ancestor| {
            // Match the return barriers used by fn_signature: closures,
            // async/const blocks and nested functions own their returns.
            ast::Fn::can_cast(ancestor.kind())
                || ast::ClosureExpr::can_cast(ancestor.kind())
                || ast::BlockExpr::cast(ancestor.clone()).is_some_and(|block| {
                    block.async_token().is_some() || block.const_token().is_some()
                })
        })
        .is_some_and(|ancestor| ancestor == *function.syntax())
}

fn error_argument(returned: &ast::ReturnExpr) -> Option<ast::Expr> {
    let call = ast::CallExpr::cast(returned.expr()?.syntax().clone())?;
    if call.expr()?.syntax().text() != "Err" {
        return None;
    }
    let list = call.arg_list()?;
    let mut arguments = list.args();
    let argument = arguments.next()?;
    arguments.next().is_none().then_some(argument)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DeltaKind, ProbeId, SourceLocation};
    use std::path::PathBuf;

    fn fixture_transition(source: &str, before: &str) -> Result<Option<(String, String)>, String> {
        let facts = crate::analysis::rust_index::summarize_file(
            PathBuf::from("src/lib.rs"),
            source.to_string(),
        );
        assert!(!facts.used_lexical_fallback);
        let owner = facts
            .functions
            .iter()
            .find(|item| item.name == "run_slow")
            .ok_or("parsed owner")?;
        let line = source
            .lines()
            .position(|text| text.contains("return Err(OpError"))
            .ok_or("changed return")?
            + 1;
        let expression =
            "return Err(OpError { code: -32800, message: \"operation cancelled\".to_string() })";
        let probe = Probe {
            id: ProbeId("scope-control".to_string()),
            location: SourceLocation::new("src/lib.rs", line, 1),
            owner: Some(owner.id.clone()),
            family: ProbeFamily::ErrorPath,
            delta: DeltaKind::Value,
            before: Some(before.to_string()),
            after: Some(expression.to_string()),
            expression: expression.to_string(),
            expected_sinks: Vec::new(),
            required_oracles: Vec::new(),
        };
        Ok(guarded_opaque_error_transition(&probe, owner))
    }

    #[test]
    fn guarded_error_transition_rejects_other_return_owners_and_ambiguous_scopes()
    -> Result<(), String> {
        let source =
            include_str!("../../../../../fixtures/error_return_unresolved_guard/input/src/lib.rs");
        let before = "return Err(cancelled_error());";
        assert!(
            fixture_transition(source, before)?.is_some(),
            "positive scope setup"
        );
        let closure = source
            .replace("if cancelled {", "let closure = || { if cancelled {")
            .replace("    }\n    Ok", "    } };\n    Ok");
        let nested = source
            .replace(
                "if cancelled {",
                "fn nested(cancelled: bool) -> Result<&'static str, OpError> { if cancelled {",
            )
            .replace("    }\n    Ok", "    } Ok(\"nested\") }\n    Ok");
        let asynchronous = source
            .replace("if cancelled {", "let future = async { if cancelled {")
            .replace("    }\n    Ok", "    } };\n    Ok");
        let async_move = source
            .replace("if cancelled {", "let future = async move { if cancelled {")
            .replace("    }\n    Ok", "    } };\n    Ok");
        let constant = source
            .replace("if cancelled {", "let value = const { if cancelled {")
            .replace("    }\n    Ok", "    } };\n    Ok");
        let alternate = source.replace("if cancelled {", "if cancelled { } else {");
        let shadow = source.replace(
            "if cancelled {",
            "let cancelled = true;\n    if cancelled {",
        );
        let statement = source
            .lines()
            .find(|text| text.contains("return Err(OpError"))
            .ok_or("return setup")?;
        let duplicate = source.replace(statement, &format!("{statement} {statement}"));
        for (name, altered) in [
            ("closure", closure),
            ("nested fn", nested),
            ("async block", asynchronous),
            ("async move block", async_move),
            ("const block", constant),
            ("else branch", alternate),
            ("shadowed guard", shadow),
            ("same-line returns", duplicate),
        ] {
            assert_ne!(altered, source, "{name} setup");
            assert!(
                fixture_transition(&altered, before)?.is_none(),
                "{name} must not borrow the outer owner's return/guard identity"
            );
        }
        assert!(
            fixture_transition(
                source,
                "let closure = || { return Err(cancelled_error()); };"
            )?
            .is_none(),
            "a removed closure return is not a removed owner return"
        );
        assert!(
            fixture_transition(
                source,
                "let future = async { return Err(cancelled_error()); };"
            )?
            .is_none(),
            "a removed async return is not a removed owner return"
        );
        assert!(
            fixture_transition(
                source,
                "let value = const { return Err(cancelled_error()); };"
            )?
            .is_none(),
            "a removed const return is not a removed owner return"
        );
        Ok(())
    }
}
