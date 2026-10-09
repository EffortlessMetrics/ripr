use super::related_tests::{
    PythonRelatedCandidate, body_calls_owner, dunder_method_class, is_python_identifier_char,
    line_prefix_looks_like_comment_or_string, test_may_reach_owner_class,
};
use super::source_utils::SourceText;
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
    // An independent active relation can still supply its own oracle. The
    // shared matcher withholds assertions and boundary inputs from controlled
    // tests, so those tests cannot lend credit to a weaker active relation.
    if !related_candidates
        .iter()
        .any(|candidate| candidate.relation.uses_oracle())
        && let Some((test, decorator)) = related_candidates.iter().find_map(|candidate| {
            super::test_activation::activation_control(candidate.test)
                .map(|decorator| (candidate.test, decorator))
        })
    {
        return Some(PythonStaticLimit {
            kind: StaticLimitKind::DecoratorIndirection,
            evidence: format!(
                "static_limit decorator_indirection: test activation for `{}` is controlled by `{decorator}`",
                test.qualified_name
            ),
            missing: format!(
                "Static limit `decorator_indirection`: related test `{}` uses `{decorator}`; static evidence does not establish that its body runs and an assertion failure fails verification. Add an independent active test or inspect the activation condition.",
                test.qualified_name
            ),
        });
    }
    if related_candidates
        .iter()
        .filter(|candidate| candidate.relation.uses_oracle())
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

/// A dunder method owner (`Cache.__setitem__`) with no related test, in a
/// workspace whose tests import the owner class or module. Python calls these
/// methods through syntax (`cache[key] = value`, `Cache(...)`) on instances the
/// adapter cannot always bind to the class — a unittest mixin's `self.Cache`
/// attribute or a test-local subclass — so "no static test path" is not a safe
/// claim. Returns `None` when a test relates, or when no test imports the
/// owner's class or module and none names the class (such a class keeps
/// `no_static_path`).
pub(super) fn implicit_dunder_dispatch_limit(
    owner: &PythonOwner,
    all_tests: &[PythonTest],
    related_candidates: &[PythonRelatedCandidate<'_>],
) -> Option<PythonStaticLimit> {
    if !related_candidates.is_empty() {
        return None;
    }
    let class = dunder_method_class(owner)?;
    all_tests
        .iter()
        .any(|test| test_may_reach_owner_class(test, owner, class))
        .then(|| PythonStaticLimit {
            kind: StaticLimitKind::DynamicDispatch,
            evidence: format!(
                "static_limit dynamic_dispatch: dunder method `{}` is invoked implicitly by Python syntax",
                owner.qualified_name
            ),
            missing: format!(
                "Static limit `dynamic_dispatch`: `{}` is a dunder method Python invokes through syntax on an instance of `{class}`; tests import `{class}`, its module or its package, or name `{class}`, but the preview adapter cannot bind the instance a test builds to the class, so it does not claim that no test reaches the owner.",
                owner.qualified_name
            ),
        })
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
        || is_imported_functools_memoization_decorator(decorator, &owner.imports)
}

fn is_functools_memoization_name(name: &str) -> bool {
    matches!(name, "lru_cache" | "cache" | "cached_property")
}

fn import_binds_stdlib_functools_module(import: &PythonImport) -> bool {
    import.imported == "functools" && import.source_module.is_empty()
}

/// `import wrappers as functools` / `from helpers import functools` rebound
/// the name; a plain `import functools` did not.
fn receiver_is_rebound_from_non_stdlib_functools(imports: &[PythonImport], receiver: &str) -> bool {
    imports
        .iter()
        .any(|import| import.alias == receiver && !import_binds_stdlib_functools_module(import))
}

/// Bare `@lru_cache` / `@cache` / `@cached_property` only when the name is
/// bound from `functools` and not also bound from another module. A
/// same-named local decorator stays limited. `@functools.lru_cache` is the
/// stdlib qualified spelling unless `functools` is rebound (`import wrappers
/// as functools` or `from helpers import functools`). `import functools as
/// ft` then `@ft.lru_cache` is the same binding unless `ft` is also rebound.
/// Call arguments are already dropped by `expr_full_name`.
fn is_imported_functools_memoization_decorator(decorator: &str, imports: &[PythonImport]) -> bool {
    if let Some((receiver, method)) = decorator.rsplit_once('.') {
        if !is_functools_memoization_name(method) {
            return false;
        }
        if receiver_is_rebound_from_non_stdlib_functools(imports, receiver) {
            return false;
        }
        if receiver == "functools" {
            return true;
        }
        return imports.iter().any(|import| {
            import_binds_stdlib_functools_module(import) && import.alias == receiver
        });
    }
    let from_functools = imports.iter().any(|import| {
        import.source_module == "functools"
            && is_functools_memoization_name(import.imported.as_str())
            && import.alias == decorator
    });
    let competing_alias = imports
        .iter()
        .any(|import| import.alias == decorator && import.source_module != "functools");
    from_functools && !competing_alias
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
    source: &SourceText<'_>,
    imports: &[PythonImport],
) -> Vec<String> {
    if !imports.iter().any(has_typer_import) {
        return Vec::new();
    }
    // One scan per file, not per owner: every owner of a Typer module asks.
    source.cli_receiver_names(scan_cli_receiver_names).to_vec()
}

fn scan_cli_receiver_names(source: &str) -> Vec<String> {
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

/// `patch.object` and `patch.multiple` substitute attributes the same way as
/// `patch` and `monkeypatch.setattr`, so they are the same runtime
/// substitution limit (#4565).
pub(super) fn test_has_mocked_module(test: &PythonTest) -> bool {
    test.decorators.iter().any(|decorator| {
        ["patch", "patch.object", "patch.multiple"]
            .iter()
            .any(|form| decorator == form || decorator.ends_with(&format!(".{form}")))
    }) || [
        "patch(",
        "patch.object(",
        "patch.multiple(",
        "monkeypatch.setattr(",
        "monkeypatch.setitem(",
        "monkeypatch.delattr(",
    ]
    .iter()
    .any(|call| body_calls_at_name_boundary(&test.body_text, call))
}

/// Whether `body` contains `call` where the name starts at an identifier
/// boundary: `mock.patch(` and `patch.object(`, not `dispatch(` or
/// `dispatch.object(`.
fn body_calls_at_name_boundary(body: &str, call: &str) -> bool {
    body.match_indices(call).any(|(idx, _)| {
        !body[..idx]
            .chars()
            .next_back()
            .is_some_and(is_python_identifier_char)
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::StaticLimitKind;
    use std::path::PathBuf;

    fn from_functools(name: &str) -> PythonImport {
        PythonImport {
            imported: name.to_string(),
            alias: name.to_string(),
            source_module: "functools".to_string(),
        }
    }

    fn import_functools() -> PythonImport {
        PythonImport {
            imported: "functools".to_string(),
            alias: "functools".to_string(),
            source_module: String::new(),
        }
    }

    fn owner_with(decorators: &[&str], imports: Vec<PythonImport>) -> PythonOwner {
        PythonOwner {
            name: "owner".to_string(),
            qualified_name: "owner".to_string(),
            file: PathBuf::from("src/owner.py"),
            start_line: 1,
            end_line: 4,
            owner_kind: None,
            decorators: decorators.iter().map(|name| (*name).to_string()).collect(),
            imports,
            cli_receiver_names: Vec::new(),
            route_paths: Vec::new(),
            dynamic_route_decorators: Vec::new(),
            parameters: Vec::new(),
            reexport_modules: Vec::new(),
            ambiguous_src_modules: Vec::new(),
            module_constants: Vec::new(),
            same_class_callees: Vec::new(),
            class_path: String::new(),
        }
    }

    fn decorator_limit(owner: &PythonOwner) -> Option<StaticLimitKind> {
        static_limit_for_change("return amount + 2", owner, &[]).map(|limit| limit.kind)
    }

    #[test]
    fn functools_memoization_decorators_do_not_limit() {
        for name in ["lru_cache", "cache", "cached_property"] {
            let qualified = format!("functools.{name}");
            assert!(
                !is_transparent_owner_decorator(&qualified),
                "qualified `{qualified}` needs owner import context"
            );
            let owner = owner_with(&[name], vec![from_functools(name)]);
            assert!(
                is_transparent_owner_decorator_for_owner(name, &owner),
                "expected bare `{name}` imported from functools to be transparent"
            );
            assert_eq!(
                decorator_limit(&owner),
                None,
                "imported `{name}` must not emit decorator_indirection"
            );
            assert_eq!(
                decorator_limit(&owner_with(&[qualified.as_str()], vec![import_functools()])),
                None,
                "`{qualified}` with `import functools` must not emit decorator_indirection"
            );
            assert_eq!(
                decorator_limit(&owner_with(&[qualified.as_str()], Vec::new())),
                None,
                "`{qualified}` with no competing import is the stdlib spelling"
            );
        }
    }

    #[test]
    fn functools_imported_from_another_module_stays_limited() {
        let owner = owner_with(
            &["functools.lru_cache"],
            vec![PythonImport {
                imported: "functools".to_string(),
                alias: "functools".to_string(),
                source_module: "helpers".to_string(),
            }],
        );
        assert_eq!(
            decorator_limit(&owner),
            Some(StaticLimitKind::DecoratorIndirection)
        );
    }

    #[test]
    fn local_lru_cache_without_functools_import_stays_limited() {
        for name in ["lru_cache", "cache", "cached_property"] {
            let owner = owner_with(&[name], Vec::new());
            assert!(
                !is_transparent_owner_decorator(name),
                "bare `{name}` without import context must stay limited"
            );
            assert_eq!(
                decorator_limit(&owner),
                Some(StaticLimitKind::DecoratorIndirection),
                "local `{name}` must keep decorator_indirection"
            );
            let other_module = owner_with(
                &[name],
                vec![PythonImport {
                    imported: name.to_string(),
                    alias: name.to_string(),
                    source_module: "helpers".to_string(),
                }],
            );
            assert_eq!(
                decorator_limit(&other_module),
                Some(StaticLimitKind::DecoratorIndirection),
                "`{name}` imported from a non-functools module must stay limited"
            );
        }
    }

    #[test]
    fn retry_and_unrelated_decorators_stay_limited_with_functools_present() {
        let functools = vec![from_functools("lru_cache"), import_functools()];
        for decorator in ["retry", "retry.with_backoff", "cache.get", "post", "wraps"] {
            let owner = owner_with(&[decorator], functools.clone());
            assert!(
                !is_transparent_owner_decorator(decorator),
                "expected `{decorator}` to remain a static limit"
            );
            assert_eq!(
                decorator_limit(&owner),
                Some(StaticLimitKind::DecoratorIndirection),
                "`{decorator}` must keep decorator_indirection even when functools is imported"
            );
        }
    }

    #[test]
    fn memoization_plus_retry_still_limits() {
        let owner = owner_with(&["lru_cache", "retry"], vec![from_functools("lru_cache")]);
        assert_eq!(
            decorator_limit(&owner),
            Some(StaticLimitKind::DecoratorIndirection)
        );
    }

    #[test]
    fn aliased_functools_module_memoization_is_transparent() {
        let owner = owner_with(
            &["ft.lru_cache"],
            vec![PythonImport {
                imported: "functools".to_string(),
                alias: "ft".to_string(),
                source_module: String::new(),
            }],
        );
        assert_eq!(decorator_limit(&owner), None);
    }

    #[test]
    fn aliased_from_functools_import_is_transparent() {
        let owner = owner_with(
            &["memoize"],
            vec![PythonImport {
                imported: "lru_cache".to_string(),
                alias: "memoize".to_string(),
                source_module: "functools".to_string(),
            }],
        );
        assert_eq!(decorator_limit(&owner), None);
    }

    #[test]
    fn functools_aliased_from_another_module_stays_limited() {
        let owner = owner_with(
            &["functools.cache"],
            vec![PythonImport {
                imported: "wrappers".to_string(),
                alias: "functools".to_string(),
                source_module: String::new(),
            }],
        );
        assert_eq!(
            decorator_limit(&owner),
            Some(StaticLimitKind::DecoratorIndirection)
        );
    }

    #[test]
    fn competing_bare_cache_import_stays_limited() {
        let owner = owner_with(
            &["cache"],
            vec![
                PythonImport {
                    imported: "cache".to_string(),
                    alias: "cache".to_string(),
                    source_module: "wrappers".to_string(),
                },
                from_functools("cache"),
            ],
        );
        assert_eq!(
            decorator_limit(&owner),
            Some(StaticLimitKind::DecoratorIndirection)
        );
    }

    #[test]
    fn competing_module_alias_stays_limited() {
        let owner = owner_with(
            &["ft.lru_cache"],
            vec![
                PythonImport {
                    imported: "wrappers".to_string(),
                    alias: "ft".to_string(),
                    source_module: String::new(),
                },
                PythonImport {
                    imported: "functools".to_string(),
                    alias: "ft".to_string(),
                    source_module: String::new(),
                },
            ],
        );
        assert_eq!(
            decorator_limit(&owner),
            Some(StaticLimitKind::DecoratorIndirection)
        );
    }
}
