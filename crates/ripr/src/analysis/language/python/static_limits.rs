use super::related_tests::{
    PythonRelatedCandidate, body_calls_owner, is_python_identifier_char,
    line_prefix_looks_like_comment_or_string,
};
use super::{PythonImport, PythonOracleShape, PythonOwner, PythonTest, split_python_assignment};
use crate::domain::{OracleStrength, StaticLimitKind};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PythonStaticLimit {
    pub(super) kind: StaticLimitKind,
    pub(super) evidence: String,
    pub(super) missing: String,
}

pub(super) fn static_limit_for_change(
    line_text: &str,
    owner: &PythonOwner,
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> Option<PythonStaticLimit> {
    let trimmed = line_text.trim();
    if contains_dynamic_dispatch(trimmed) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::DynamicDispatch,
            evidence: "static_limit dynamic_dispatch: changed line uses dynamic call dispatch"
                .to_string(),
            missing: "Static limit `dynamic_dispatch`: the Python preview adapter saw a dynamic call shape such as `getattr(...)` or `registry[key](...)`; syntax alone cannot resolve the called behavior.".to_string(),
        });
    }
    if contains_dynamic_import(trimmed) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::MissingImportGraph,
            evidence: "static_limit missing_import_graph: changed line uses dynamic import syntax"
                .to_string(),
            missing: "Static limit `missing_import_graph`: the changed line uses dynamic import syntax such as `importlib.import_module(...)` or `__import__(...)`; the Python preview adapter does not build an import graph or resolve imported implementation semantics.".to_string(),
        });
    }
    if contains_metaprogramming(trimmed) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::Metaprogramming,
            evidence: "static_limit metaprogramming: changed line uses metaprogramming syntax"
                .to_string(),
            missing: "Static limit `metaprogramming`: the Python preview adapter saw metaprogramming syntax and does not infer runtime-created behavior.".to_string(),
        });
    }
    if let Some(decorator) = owner.dynamic_route_decorators.first() {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::DecoratorIndirection,
            evidence: format!(
                "static_limit decorator_indirection: dynamic_route_registration `{decorator}`"
            ),
            missing: format!(
                "Static limit `dynamic_route_registration`: owner `{}` uses dynamic route registration `{decorator}`; syntax-first preview evidence cannot safely match client calls to a concrete route path.",
                owner.qualified_name
            ),
        });
    }
    if let Some(decorator) = owner
        .decorators
        .iter()
        .find(|decorator| !is_transparent_owner_decorator_for_owner(decorator, owner))
    {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::DecoratorIndirection,
            evidence: format!("static_limit decorator_indirection: `{decorator}`"),
            missing: format!(
                "Static limit `decorator_indirection`: owner `{}` is decorated with `{decorator}`; syntax-first preview evidence does not resolve decorator-modified call behavior.",
                owner.qualified_name
            ),
        });
    }
    if related_candidates
        .iter()
        .any(|candidate| test_has_mocked_module(candidate.test))
    {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::MockedModule,
            evidence: "static_limit mocked_module: related test uses patch/mock/monkeypatch module syntax"
                .to_string(),
            missing: "Static limit `mocked_module`: a related Python test uses patch/mock/monkeypatch module syntax; the preview adapter does not resolve runtime substitution semantics.".to_string(),
        });
    }
    if related_candidates_have_property_based_test_limit(related_candidates) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::PropertyBasedTest,
            evidence: "static_limit property_based_test: related test uses generated inputs"
                .to_string(),
            missing: "Static limit `property_based_test`: a related Python test uses property-based generated inputs such as `@given(...)`; syntax-first preview evidence cannot prove whether the generated cases include the changed discriminator.".to_string(),
        });
    }
    if related_candidates_have_unresolved_pytest_fixture_limit(owner, related_candidates) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::UnresolvedPytestFixture,
            evidence:
                "static_limit unresolved_pytest_fixture: related test uses fixture-sourced values"
                    .to_string(),
            missing: "Static limit `unresolved_pytest_fixture`: a related pytest test depends on fixture-sourced values; syntax-first preview evidence cannot prove whether the fixture supplies the changed discriminator or expected value.".to_string(),
        });
    }
    if related_candidates_have_opaque_custom_assertion_limit(related_candidates) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::OpaqueCustomAssertionHelper,
            evidence: "static_limit opaque_custom_assertion_helper: related test uses an opaque custom assertion helper"
                .to_string(),
            missing: "Static limit `opaque_custom_assertion_helper`: a related Python test uses a custom assertion helper such as `assert_*(...)`; the preview adapter cannot inspect the helper body or determine whether it already observes the changed discriminator.".to_string(),
        });
    }
    if line_uses_imported_symbol(trimmed, &owner.imports) {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::MissingImportGraph,
            evidence: "static_limit missing_import_graph: changed line calls an imported symbol"
                .to_string(),
            missing: "Static limit `missing_import_graph`: the changed line calls an imported symbol; the Python preview adapter does not build an import graph or resolve imported implementation semantics.".to_string(),
        });
    }
    if trimmed.contains("lambda ") {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::UnsupportedSyntax,
            evidence: "static_limit unsupported_syntax: changed line uses lambda syntax"
                .to_string(),
            missing: "Static limit `unsupported_syntax`: the changed line uses a Python syntax shape this preview adapter does not model precisely yet.".to_string(),
        });
    }
    None
}

pub(super) fn contains_dynamic_dispatch(text: &str) -> bool {
    text.contains("getattr(") || (text.contains('[') && text.contains("]("))
}

pub(super) fn contains_dynamic_import(text: &str) -> bool {
    contains_python_call_shape(text, "importlib.import_module")
        || contains_python_call_shape(text, "__import__")
}

pub(super) fn contains_python_call_shape(text: &str, callee: &str) -> bool {
    text.match_indices(callee).any(|(idx, _)| {
        python_callee_start_has_boundary(text, idx)
            && text[idx + callee.len()..].trim_start().starts_with('(')
            && !python_prefix_hides_code(line_prefix_before(text, idx))
    })
}

pub(super) fn python_callee_start_has_boundary(text: &str, idx: usize) -> bool {
    text[..idx]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_python_identifier_char(ch) && ch != '.')
}

pub(super) fn line_prefix_before(text: &str, idx: usize) -> &str {
    text[..idx]
        .rsplit_once('\n')
        .map_or(&text[..idx], |(_, line)| line)
}

pub(super) fn python_prefix_hides_code(prefix: &str) -> bool {
    let mut quote = None;
    let mut escaped = false;
    for ch in prefix.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if let Some(active_quote) = quote {
            if ch == '\\' {
                escaped = true;
            } else if ch == active_quote {
                quote = None;
            }
            continue;
        }
        if ch == '#' {
            return true;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
        }
    }
    quote.is_some()
}

pub(super) fn contains_metaprogramming(text: &str) -> bool {
    text.contains("__getattr__")
        || text.contains("type(")
        || text.contains("setattr(")
        || contains_metaclass_declaration(text)
}

fn contains_metaclass_declaration(text: &str) -> bool {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("class ") {
        return false;
    }
    contains_python_keyword_assignment_shape(text, "metaclass")
}

fn contains_python_keyword_assignment_shape(text: &str, keyword: &str) -> bool {
    text.match_indices(keyword).any(|(idx, _)| {
        python_callee_start_has_boundary(text, idx)
            && text[idx + keyword.len()..].trim_start().starts_with('=')
            && !python_prefix_hides_code(line_prefix_before(text, idx))
    })
}

pub(super) fn is_transparent_owner_decorator(decorator: &str) -> bool {
    decorator == "staticmethod"
        || decorator == "classmethod"
        || decorator == "async_def"
        || is_static_route_decorator(decorator)
        || is_static_cli_decorator(decorator)
}

pub(super) fn is_transparent_owner_decorator_for_owner(
    decorator: &str,
    owner: &PythonOwner,
) -> bool {
    is_transparent_owner_decorator(decorator)
        || is_static_cli_decorator_with_import_context(
            decorator,
            &owner.imports,
            &owner.cli_receiver_names,
        )
}

pub(super) fn is_static_route_decorator(decorator: &str) -> bool {
    let Some((receiver, method)) = decorator.rsplit_once('.') else {
        return false;
    };
    if !matches!(
        method,
        "get"
            | "post"
            | "put"
            | "patch"
            | "delete"
            | "options"
            | "head"
            | "route"
            | "api_route"
            | "websocket"
    ) {
        return false;
    }

    let Some(receiver_name) = receiver.rsplit('.').next() else {
        return false;
    };
    matches!(
        receiver_name,
        "app" | "api" | "router" | "routes" | "bp" | "blueprint"
    ) || receiver_name.ends_with("_app")
        || receiver_name.ends_with("_api")
        || receiver_name.ends_with("_router")
        || receiver_name.ends_with("_routes")
        || receiver_name.ends_with("_bp")
}

fn is_static_cli_decorator(decorator: &str) -> bool {
    matches!(
        decorator,
        "click.command"
            | "click.group"
            | "click.option"
            | "click.argument"
            | "typer.command"
            | "typer.callback"
    )
}

fn is_static_cli_decorator_with_import_context(
    decorator: &str,
    imports: &[PythonImport],
    cli_receiver_names: &[String],
) -> bool {
    if is_static_cli_decorator(decorator) {
        return true;
    }
    let Some((receiver, method)) = decorator.rsplit_once('.') else {
        return false;
    };
    if !matches!(method, "command" | "callback") || !imports.iter().any(has_typer_import) {
        return false;
    }
    let receiver_name = receiver.rsplit('.').next().unwrap_or(receiver);
    cli_receiver_names
        .iter()
        .any(|candidate| candidate == receiver_name)
}

fn has_typer_import(import: &PythonImport) -> bool {
    import.imported == "typer" && import.alias == "typer"
}

pub(super) fn collect_static_cli_receiver_names(
    source: &str,
    imports: &[PythonImport],
) -> Vec<String> {
    if !imports.iter().any(has_typer_import) {
        return Vec::new();
    }
    let mut receivers: Vec<String> = source
        .lines()
        .filter_map(|line| {
            let text = line.trim();
            let (lhs, rhs) = split_python_assignment(text)?;
            if !is_simple_python_identifier(lhs) || !contains_python_call_shape(rhs, "typer.Typer")
            {
                return None;
            }
            Some(lhs.to_string())
        })
        .collect();
    receivers.sort();
    receivers.dedup();
    receivers
}

pub(super) fn is_simple_python_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

pub(super) fn test_has_mocked_module(test: &PythonTest) -> bool {
    test.decorators
        .iter()
        .any(|decorator| decorator == "patch" || decorator.ends_with(".patch"))
        || test.body_text.contains("patch(")
        || test.body_text.contains(".patch(")
        || test.body_text.contains("monkeypatch.setattr(")
        || test.body_text.contains("monkeypatch.setitem(")
        || test.body_text.contains("monkeypatch.delattr(")
}

fn related_candidates_have_property_based_test_limit(
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> bool {
    related_candidates
        .iter()
        .filter(|candidate| candidate.relation.uses_oracle())
        .any(|candidate| {
            test_uses_property_based_inputs(candidate.test)
                && !candidate.test.assertions.iter().any(|assertion| {
                    assertion.oracle_strength.rank() >= OracleStrength::Strong.rank()
                })
        })
}

fn test_uses_property_based_inputs(test: &PythonTest) -> bool {
    test.decorators.iter().any(|decorator| {
        decorator == "given"
            || decorator.ends_with(".given")
            || decorator == "hypothesis.given"
            || decorator == "example"
            || decorator.ends_with(".example")
    })
}

fn related_candidates_have_unresolved_pytest_fixture_limit(
    owner: &PythonOwner,
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> bool {
    let mut has_unresolved_fixture_relation = false;
    let mut has_concrete_oracle_relation = false;

    for candidate in related_candidates
        .iter()
        .filter(|candidate| candidate.relation.uses_oracle())
    {
        if test_has_unresolved_pytest_fixture_inputs(candidate.test, owner) {
            has_unresolved_fixture_relation = true;
        } else {
            has_concrete_oracle_relation = true;
        }
    }

    has_unresolved_fixture_relation && !has_concrete_oracle_relation
}

fn test_has_unresolved_pytest_fixture_inputs(test: &PythonTest, owner: &PythonOwner) -> bool {
    test.framework == "pytest"
        && !test.parametrized
        && body_calls_owner(&test.body_text, owner)
        && test
            .fixtures
            .iter()
            .filter(|fixture| !is_known_auxiliary_pytest_fixture(fixture))
            .any(|fixture| body_uses_identifier(&test.body_text, fixture))
}

fn is_known_auxiliary_pytest_fixture(fixture: &str) -> bool {
    matches!(
        fixture,
        "capfd"
            | "capfdbinary"
            | "caplog"
            | "capsys"
            | "capsysbinary"
            | "client"
            | "monkeypatch"
            | "mocker"
            | "record_property"
            | "record_testsuite_property"
            | "recwarn"
            | "test_client"
            | "tmp_path"
            | "tmp_path_factory"
            | "tmpdir"
            | "tmpdir_factory"
    )
}

fn body_uses_identifier(body_text: &str, identifier: &str) -> bool {
    let mut line_start = 0;
    for line in body_text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if !trimmed.starts_with('@')
            && !trimmed.starts_with("def ")
            && !trimmed.starts_with("async def ")
            && line.match_indices(identifier).any(|(idx, _)| {
                let body_idx = line_start + idx;
                has_identifier_boundary(body_text, body_idx, identifier.len())
                    && !line_prefix_looks_like_comment_or_string(body_text, body_idx)
            })
        {
            return true;
        }
        line_start += line.len();
    }
    false
}

pub(super) fn has_identifier_boundary(body_text: &str, idx: usize, len: usize) -> bool {
    let before = body_text[..idx].chars().next_back();
    let after = body_text[idx + len..].chars().next();
    before.is_none_or(|ch| !is_python_identifier_char(ch))
        && after.is_none_or(|ch| !is_python_identifier_char(ch))
}

fn related_candidates_have_opaque_custom_assertion_limit(
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> bool {
    let mut has_opaque_helper = false;
    let mut has_known_strong_oracle = false;

    for candidate in related_candidates
        .iter()
        .filter(|candidate| candidate.relation.uses_oracle())
    {
        for assertion in &candidate.test.assertions {
            if assertion.oracle_shape == PythonOracleShape::UnknownCustomHelper {
                has_opaque_helper = true;
            } else if assertion.oracle_strength.rank() >= OracleStrength::Strong.rank() {
                has_known_strong_oracle = true;
            }
        }
    }

    has_opaque_helper && !has_known_strong_oracle
}

pub(super) fn line_uses_imported_symbol(text: &str, imports: &[PythonImport]) -> bool {
    imports.iter().any(|import| {
        !is_known_mock_constructor_import(import)
            && !line_uses_known_static_cli_symbol(text, import)
            && (text.contains(&format!("{}(", import.alias))
                || text.contains(&format!("{}.", import.alias)))
    })
}

pub(super) fn is_known_mock_constructor_import(import: &PythonImport) -> bool {
    matches!(import.imported.as_str(), "Mock" | "MagicMock")
        || matches!(import.alias.as_str(), "Mock" | "MagicMock")
}

fn line_uses_known_static_cli_symbol(text: &str, import: &PythonImport) -> bool {
    let alias = import.alias.as_str();
    match import.imported.as_str() {
        "click" if alias == "click" => contains_python_call_shape(text, "click.echo"),
        "typer" if alias == "typer" => contains_python_call_shape(text, "typer.echo"),
        "sys" if alias == "sys" => {
            contains_python_call_shape(text, "sys.exit")
                || contains_python_call_shape(text, "sys.stdout.write")
                || contains_python_call_shape(text, "sys.stderr.write")
        }
        _ => false,
    }
}
