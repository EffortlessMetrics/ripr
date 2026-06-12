//! Static-limit analysis for the TypeScript preview adapter.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptStaticLimit {
    pub(crate) kind: StaticLimitKind,
    pub(crate) evidence: Vec<String>,
    pub(crate) missing: String,
    pub(crate) repair_route: String,
}

pub(crate) fn static_limit_for_change(
    line_text: &str,
    owner: &TypeScriptOwner,
    mock_paths: &[String],
) -> Option<TypeScriptStaticLimit> {
    let trimmed = line_text.trim();
    if is_computed_member_call(trimmed) {
        return Some(TypeScriptStaticLimit {
            kind: StaticLimitKind::DynamicDispatch,
            evidence: vec![
                "static_limit dynamic_dispatch: changed line uses computed member invocation"
                    .to_string(),
            ],
            missing: "Static limit `dynamic_dispatch`: the TypeScript preview adapter saw a computed member call such as `obj[name](...)`; syntax alone cannot resolve the called behavior. Repair route: inspect the concrete dispatch key or add analyzer support for explicit dispatch-map resolution before issuing a repair packet.".to_string(),
            repair_route: "Repair route: inspect the concrete dispatch key or add analyzer support for explicit dispatch-map resolution before issuing a repair packet.".to_string(),
        });
    }
    if contains_metaprogramming(trimmed) {
        return Some(TypeScriptStaticLimit {
            kind: StaticLimitKind::Metaprogramming,
            evidence: vec![
                "static_limit metaprogramming: changed line uses metaprogramming syntax"
                    .to_string(),
            ],
            missing: "Static limit `metaprogramming`: the TypeScript preview adapter saw Proxy, Reflect, or property-definition metaprogramming syntax and does not infer runtime-created behavior. Repair route: add metaprogramming-aware modeling or keep the finding as human-review-only before issuing a repair packet.".to_string(),
            repair_route: "Repair route: add metaprogramming-aware modeling or keep the finding as human-review-only before issuing a repair packet.".to_string(),
        });
    }
    if owner.decorated || trimmed.starts_with('@') {
        return Some(TypeScriptStaticLimit {
            kind: StaticLimitKind::DecoratorIndirection,
            evidence: vec![format!(
                "static_limit decorator_indirection: owner `{}` uses TypeScript decorators",
                owner.name
            )],
            missing: format!(
                "Static limit `decorator_indirection`: owner `{}` uses TypeScript decorators; syntax-first preview evidence does not resolve decorator-modified call behavior. Repair route: add decorator-aware owner modeling or verify decorator-modified behavior manually before issuing a repair packet.",
                owner.name
            ),
            repair_route: "Repair route: add decorator-aware owner modeling or verify decorator-modified behavior manually before issuing a repair packet.".to_string(),
        });
    }
    if !mock_paths.is_empty() {
        let preview: String = mock_paths
            .iter()
            .map(|path| format!("`{path}`"))
            .collect::<Vec<_>>()
            .join(", ");
        return Some(TypeScriptStaticLimit {
            kind: StaticLimitKind::MockedModule,
            evidence: mock_paths
                .iter()
                .map(|path| format!("static_limit mocked_module: `{path}`"))
                .collect(),
            missing: format!(
                "Static limit `mocked_module`: related test file mocks {preview} via `vi.mock(...)` / `jest.mock(...)`. The TypeScript preview adapter does not resolve mocked module semantics, so the substitution under test is opaque to static evidence. Repair route: add mock-shape support or validate the real substitution under test before issuing a repair packet."
            ),
            repair_route: "Repair route: add mock-shape support or validate the real substitution under test before issuing a repair packet.".to_string(),
        });
    }
    if let Some(import) = imported_symbol_call(trimmed, &owner.imports) {
        let symbol = if import.namespace {
            format!("{}.*", import.local)
        } else {
            import.local.clone()
        };
        return Some(TypeScriptStaticLimit {
            kind: StaticLimitKind::MissingImportGraph,
            evidence: vec![format!(
                "static_limit missing_import_graph: changed line calls imported symbol `{symbol}`"
            )],
            missing: format!(
                "Static limit `missing_import_graph`: the changed line calls imported symbol `{symbol}` from `{}`; the TypeScript preview adapter does not build a package or import graph for production implementation semantics. Repair route: add import graph support or inspect the imported implementation before issuing a repair packet.",
                import.source
            ),
            repair_route: "Repair route: add import graph support or inspect the imported implementation before issuing a repair packet.".to_string(),
        });
    }
    None
}

pub(crate) fn contains_metaprogramming(text: &str) -> bool {
    [
        "new Proxy(",
        "Proxy(",
        "Reflect.",
        "Object.defineProperty(",
        "Object.defineProperties(",
    ]
    .iter()
    .any(|shape| contains_unquoted_shape(text, shape))
}

pub(crate) fn imported_symbol_call<'a>(
    line_text: &str,
    imports: &'a [TypeScriptImport],
) -> Option<&'a TypeScriptImport> {
    imports.iter().find(|import| {
        if import.namespace {
            contains_namespace_import_call(line_text, &import.local)
        } else {
            contains_call_name(line_text, &import.local)
        }
    })
}

pub(crate) fn contains_namespace_import_call(line_text: &str, namespace: &str) -> bool {
    let needle = format!("{namespace}.");
    line_text.match_indices(&needle).any(|(idx, _)| {
        has_member_call_boundary(line_text, idx)
            && !line_prefix_looks_like_comment_or_string(line_text, idx)
            && !inside_block_comment(line_text, idx)
            && line_text
                .get(idx + needle.len()..)
                .is_some_and(namespace_tail_has_call)
    })
}

pub(crate) fn namespace_tail_has_call(tail: &str) -> bool {
    let mut saw_name = false;
    for ch in tail.chars() {
        if ch == '(' {
            return saw_name;
        }
        if ch.is_whitespace() || ch == ';' || ch == ',' || ch == ')' || ch == ']' || ch == '}' {
            return false;
        }
        if ch == '?' || ch == '.' {
            continue;
        }
        if is_javascript_identifier_char(ch) {
            saw_name = true;
            continue;
        }
        return false;
    }
    false
}
