//! Static-limit analysis for the TypeScript preview adapter.
//!
//! Named limitation taxonomy (RIPR-SPEC-0085 §PR4):
//! Each limitation variant emits `typescript_limitation: <name>` as an additive
//! evidence line. Only limitations with a REAL producer are emitted here.
//! Producer status for named limitations owned by this module:
//!
//! - `typescript_table_case_unresolved` — LANDED in table-case oracle hardening
//! - `typescript_dynamic_assertion_unresolved` — LANDED in oracle hardening
//! - `typescript_oracle_helper_gated` — LANDED in helper-gated oracle disclosure
//! - `typescript_target_unresolved` — LANDED in PR 6 (cross-package ownership detection)
//! - `typescript_path_alias_unresolved` — LANDED in RIPR-SPEC-0099 (tsconfig path alias gap)

use super::*;

/// A named TypeScript limitation derived from a real detected TypeScript construct.
///
/// Each limitation carries the taxonomy `name` (the `typescript_limitation: <name>`
/// token), a `sample_source` pointing at the real AST evidence (`file:line`),
/// a `why_not_actionable` rationale, and a `repair_route` pointer to the analyzer
/// backlog. Emitted as additive evidence lines — the existing `static_limit_kind`
/// field is NOT changed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TypeScriptNamedLimitation {
    /// Taxonomy name, e.g. `"typescript_mock_only_observer"`.
    pub(crate) name: &'static str,
    /// `file:line` from the real AST evidence that triggered this limitation.
    pub(crate) sample_source: String,
    /// Human-readable reason why the finding is not actionable.
    pub(crate) why_not_actionable: String,
    /// Pointer to the analyzer backlog slice that would resolve this.
    pub(crate) repair_route: &'static str,
}

impl TypeScriptNamedLimitation {
    /// Emit the four additive evidence lines for this limitation.
    ///
    /// Produces (in order):
    /// - `typescript_limitation: <name>`
    /// - `typescript_limitation_sample: <name> at <sample_source>`
    /// - `typescript_limitation_why: <name> — <why_not_actionable>`
    /// - `typescript_limitation_repair_route: <name> → <repair_route>`
    pub(crate) fn evidence_lines(&self) -> Vec<String> {
        vec![
            format!("typescript_limitation: {}", self.name),
            format!(
                "typescript_limitation_sample: {} at {}",
                self.name, self.sample_source
            ),
            format!(
                "typescript_limitation_why: {} — {}",
                self.name, self.why_not_actionable
            ),
            format!(
                "typescript_limitation_repair_route: {} → {}",
                self.name, self.repair_route
            ),
        ]
    }
}

/// Map a `StaticLimitKind` producer to its TS-specific named limitation, if any.
///
/// Only `MockedModule` and `MissingImportGraph` are mapped here because they
/// fire from real TypeScript-specific detected constructs (`jest.mock`/`vi.mock`
/// calls and import-graph calls respectively). The other `StaticLimitKind`
/// variants (`DynamicDispatch`, `Metaprogramming`, `DecoratorIndirection`) are
/// language-agnostic and do not have a TS-specific named limitation in the
/// PR 4 taxonomy.
///
/// `file` and `line` are the changed source location used as the `sample_source`.
pub(crate) fn named_limitation_for_static_limit(
    limit: &TypeScriptStaticLimit,
    file: &Path,
    line: usize,
) -> Option<TypeScriptNamedLimitation> {
    let sample_source = format!("{}:{}", normalized_path(file), line);
    match limit.kind {
        StaticLimitKind::MockedModule => Some(TypeScriptNamedLimitation {
            name: "typescript_mock_only_observer",
            sample_source,
            why_not_actionable: "the related test file uses jest.mock/vi.mock; the adapter cannot resolve what the mock substitutes so the observed behavior is opaque".to_string(),
            repair_route: "analysis/typescript-mock-shape-resolution",
        }),
        StaticLimitKind::MissingImportGraph => Some(TypeScriptNamedLimitation {
            name: "typescript_import_graph_unresolved",
            sample_source,
            why_not_actionable: "the changed line calls an imported symbol whose implementation is not available to the syntax-only adapter; cross-module dispatch is opaque without an import graph".to_string(),
            repair_route: "analysis/typescript-import-graph",
        }),
        _ => None,
    }
}

/// Collect `typescript_target_unresolved` limitations from tests that reference
/// the owner by name but whose ownership cannot be resolved.
///
/// Real producers:
///
/// 1. **Cross-package exclusion**: a test in a *different* package references
///    the owner by call name (`contains_call_name`) or via an import.  The
///    test would have produced an `ImportedOwnerCall` or `DirectOwnerCall`
///    relation, but the package-local filter discarded it.  We detect this by
///    comparing candidates with vs. without the package-local filter.
///
/// 2. **Direct-call with no resolvable import**: `contains_call_name` is true
///    in a test outside the owner's file, but no import in that test resolves
///    to the owner source — the adapter cannot confirm ownership.
///
/// INVARIANT: this function is ONLY called when `workspace_root` is `Some`
/// (i.e. when the package-local filter is active).  It is NOT called
/// speculatively.
pub(crate) fn named_limitations_for_unresolved_ownership(
    owner: &TypeScriptOwner,
    all_tests: &[TypeScriptTest],
    workspace_root: &Path,
) -> Vec<TypeScriptNamedLimitation> {
    let mut limitations: Vec<TypeScriptNamedLimitation> = Vec::new();
    let mut saw_target_unresolved = false;

    for test in all_tests {
        if saw_target_unresolved {
            break;
        }
        // Only consider tests that are NOT in the same package — cross-package
        // ones are the real producer.  Within-package tests are handled by the
        // normal candidate logic.
        if same_package_root(&owner.file, &test.file, workspace_root) {
            continue;
        }
        // Check whether this cross-package test actually references the owner
        // by a call or import — i.e. it WOULD have been a candidate if the
        // package-local filter were absent.
        let has_owner_reference = contains_call_name(&test.body_text, &owner.name)
            || test.imports_in_file.iter().any(|import| {
                import_source_matches_owner_text(import, &test.file, owner)
                    && import_references_owner_by_name(import, &test.body_text, owner)
            });

        if !has_owner_reference {
            continue;
        }

        let sample_source = format!("{}:{}", normalized_path(&test.file), test.line);
        limitations.push(TypeScriptNamedLimitation {
            name: "typescript_target_unresolved",
            sample_source,
            why_not_actionable: format!(
                "test `{}` in `{}` references owner `{}` by name but is in a different package \
                 (`{}`); cross-package ownership cannot be resolved without an import graph \
                 — the adapter cannot confirm this test observes the changed source",
                test.name,
                normalized_path(&test.file),
                owner.name,
                normalized_path(&test.file)
                    .split('/')
                    .take(2)
                    .collect::<Vec<_>>()
                    .join("/"),
            ),
            repair_route: "analysis/typescript-cross-package-ownership",
        });
        saw_target_unresolved = true;
    }
    limitations
}

/// Collect `typescript_path_alias_unresolved` limitations from tests whose
/// non-relative imports plausibly target the owner but could not be resolved.
///
/// This is the ALWAYS-ON honesty disclosure per RIPR-SPEC-0099:
///
/// Fires when ALL of the following hold for at least one import in a test:
/// 1. The import source is NON-RELATIVE (does not start with `./` or `../`).
/// 2. The imported symbol name matches the owner's exported name (name-matched).
/// 3. The import source did NOT resolve to the owner file (no credit given).
///
/// Condition (3) covers both the flag-OFF path (no alias map) AND the flag-ON
/// but ambiguous path (alias map present but no unique file found).
///
/// The limitation is CLASSIFICATION-NEUTRAL: it does NOT flip `no_static_path`
/// to `exposed`; it only adds disclosure evidence explaining why the path may
/// be incomplete.  Do NOT emit it for ordinary third-party imports (e.g.
/// `lodash`, `react`) where the imported name does NOT match the owner name.
///
/// `owner_was_credited` is true when the test was already included in
/// `related` (i.e. resolution succeeded); in that case we must NOT emit the
/// limitation for the same import (we would be disclosing a gap that isn't
/// there).
pub(crate) fn named_limitations_for_alias_unresolved(
    owner: &TypeScriptOwner,
    all_tests: &[TypeScriptTest],
    owner_was_credited: impl Fn(&TypeScriptTest) -> bool,
) -> Vec<TypeScriptNamedLimitation> {
    let mut limitations: Vec<TypeScriptNamedLimitation> = Vec::new();
    let mut saw = false;

    for test in all_tests {
        if saw {
            break;
        }
        if owner_was_credited(test) {
            // Already credited — no gap to disclose.
            continue;
        }
        for import in &test.imports_in_file {
            // Only non-relative specifiers are candidates for alias gap.
            if import.source.starts_with("./") || import.source.starts_with("../") {
                continue;
            }
            // Name-matched: the imported symbol must match the owner's name.
            let name_matches = match &import.imported {
                Some(name) => name == &owner.name || name == "default",
                None if import.namespace => {
                    // Namespace import: name match is always possible; accept.
                    true
                }
                None => false,
            };
            // Default-import name check: only if the local binding name matches.
            // For namespace imports, always consider as plausible.
            let plausible_owner_import = if import.namespace {
                // `import * as X from '@/module'` — plausible if X.ownerName is called
                false // namespace imports don't pinpoint a single name — skip
            } else {
                name_matches
            };
            if !plausible_owner_import {
                continue;
            }
            // This import is name-matched and non-relative and did not credit the owner.
            let sample_source = format!("{}:{}", normalized_path(&test.file), test.line);
            limitations.push(TypeScriptNamedLimitation {
                name: "typescript_path_alias_unresolved",
                sample_source,
                why_not_actionable: format!(
                    "test `{}` imports `{}` from non-relative specifier `{}` \
                     which plausibly targets owner `{}` in `{}`, but the adapter \
                     could not resolve the specifier to a unique workspace file \
                     without a tsconfig.json alias map; enable `[typescript] \
                     resolve_tsconfig_paths = true` for credit",
                    test.name,
                    import.imported.as_deref().unwrap_or(&owner.name),
                    import.source,
                    owner.name,
                    normalized_path(&owner.file),
                ),
                repair_route: "analysis/typescript-tsconfig-path-alias-resolution",
            });
            saw = true;
            break;
        }
    }
    limitations
}

/// Check whether an import source (relative path) resolves to the same module
/// path as the owner file, given the test file's location.
///
/// Unlike `import_source_matches_owner` in `related_tests.rs`, this helper is
/// a pure string computation — it does not need to be in the same module.
fn import_source_matches_owner_text(
    import: &TypeScriptImport,
    test_file: &Path,
    owner: &TypeScriptOwner,
) -> bool {
    normalized_relative_import_module_standalone(test_file, &import.source)
        .is_some_and(|module| module == normalized_module_path_standalone(&owner.file))
}

fn import_references_owner_by_name(
    import: &TypeScriptImport,
    body_text: &str,
    owner: &TypeScriptOwner,
) -> bool {
    if import.namespace {
        contains_member_call_name(body_text, &import.local, &owner.name)
    } else {
        import.imported.as_deref() == Some(owner.name.as_str())
            && contains_call_name(body_text, &import.local)
    }
}

fn normalized_relative_import_module_standalone(test_file: &Path, source: &str) -> Option<String> {
    if !source.starts_with("./") && !source.starts_with("../") {
        return None;
    }
    let mut parts = normalized_path(test_file.parent().unwrap_or_else(|| Path::new("")))
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let normalized_source = source.replace('\\', "/");
    for part in normalized_source.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part.to_string()),
        }
    }
    Some(strip_ts_extension(&parts.join("/")))
}

fn normalized_module_path_standalone(path: &Path) -> String {
    strip_ts_extension(&normalized_path(path))
}

fn strip_ts_extension(path: &str) -> String {
    for suffix in [".tsx", ".ts", ".jsx", ".js"] {
        if let Some(stripped) = path.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    path.to_string()
}

fn contains_member_call_name(body_text: &str, object_name: &str, method_name: &str) -> bool {
    let needle = format!("{object_name}.{method_name}(");
    body_text.match_indices(&needle).any(|(idx, _)| {
        has_member_call_boundary(body_text, idx)
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
            && !inside_block_comment(body_text, idx)
    })
}

/// Collect named oracle-based limitations from oracle-eligible related candidates.
///
/// Scans oracle-eligible related tests for real producers:
///
/// - assertion-helper call wrapping the changed owner call with no direct
///   extracted assertion → `typescript_oracle_helper_gated`
///   Real producer: the test body contains an `assert*` / `expect*` helper call
///   whose argument list includes the changed owner call, but the syntax-first
///   assertion extractor found no supported direct assertion. This is a
///   limitation only; the helper's semantics are not credited.
///
/// - `OracleKind::Snapshot` → `typescript_snapshot_discriminator_unresolved`
///   Real producer: `oracle.rs::oracle_for_matcher` already recognises
///   `toMatchSnapshot`/`toMatchInlineSnapshot` as `OracleKind::Snapshot`.
///
/// - `OracleKind::Unknown` with a non-empty matcher string →
///   `typescript_custom_matcher_unresolved`
///   Real producer: any `expect(x).<matcher>(...)` call whose matcher name is
///   NOT in `oracle.rs`'s recognised set returns `OracleKind::Unknown`. The
///   matcher string is real AST evidence from the oxc-parsed call expression.
///
/// - `has_dynamic_matcher_arg = true` → `typescript_dynamic_assertion_unresolved`
///   Real producer: `oracle.rs::extract_matcher_expected_value` returns
///   `has_dynamic_matcher_arg = true` when the matcher argument is a
///   non-literal dynamic expression (a variable, function call, or computed
///   value). This is a PR 5 addition — the producer now exists.
///
/// - table-form test with `has_dynamic_matcher_arg = true` →
///   `typescript_table_case_unresolved`
///   Real producer: `tests_extract.rs::test_callee_is_each` accepts array-form
///   `test.each(...)` / `it.each(...)` calls and stores the call source in
///   `TypeScriptTest::body_text`. When the matcher expected value is a row
///   variable, syntax-only preview evidence cannot bind the row to a concrete
///   expected value.
///
pub(crate) fn named_limitations_for_oracle_candidates(
    owner: &TypeScriptOwner,
    candidates: &[TypeScriptRelatedCandidate<'_>],
) -> Vec<TypeScriptNamedLimitation> {
    // Only consider oracle-eligible candidates (direct call, imported call, etc.)
    // Heuristic-only (name/proximity) links are not oracle-eligible and cannot
    // produce oracle-based limitation evidence.
    let mut limitations: Vec<TypeScriptNamedLimitation> = Vec::new();
    let mut saw_oracle_helper_gated = false;
    let mut saw_snapshot = false;
    let mut saw_custom_matcher = false;
    let mut saw_table_case = false;
    let mut saw_dynamic_assertion = false;

    for candidate in candidates.iter().filter(|c| c.relation.uses_oracle()) {
        if !saw_oracle_helper_gated
            && candidate.test.assertions.is_empty()
            && let Some((helper_name, helper_line)) =
                oracle_helper_gated_call(owner, candidate.test)
        {
            let sample = format!("{}:{}", normalized_path(&candidate.test.file), helper_line);
            let why = format!(
                "the test calls assertion helper `{helper_name}(...)` around owner `{}`; the adapter cannot inspect the helper body or prove its oracle semantics from the test call site",
                owner.name
            );
            limitations.push(TypeScriptNamedLimitation {
                name: "typescript_oracle_helper_gated",
                sample_source: sample,
                why_not_actionable: why,
                repair_route: "analysis/typescript-oracle-helper-resolution",
            });
            saw_oracle_helper_gated = true;
        }
        for assertion in &candidate.test.assertions {
            // Snapshot limitation: `toMatchSnapshot` / `toMatchInlineSnapshot`
            if matches!(assertion.oracle_kind, OracleKind::Snapshot) && !saw_snapshot {
                let sample = format!(
                    "{}:{}",
                    normalized_path(&candidate.test.file),
                    assertion.line
                );
                limitations.push(TypeScriptNamedLimitation {
                    name: "typescript_snapshot_discriminator_unresolved",
                    sample_source: sample,
                    why_not_actionable: "the test uses toMatchSnapshot/toMatchInlineSnapshot; snapshot oracles do not pin a specific discriminator value so the changed behavior may not be caught by a stale snapshot".to_string(),
                    repair_route: "analysis/typescript-snapshot-oracle-hardening",
                });
                saw_snapshot = true;
            }
            // Custom matcher limitation: Unknown oracle with a real, non-empty matcher string.
            // `oracle.rs::oracle_for_matcher` returns `OracleKind::Unknown` for any matcher
            // not in the recognised set. The matcher field is the real AST identifier from the
            // `expect(x).<matcher>(...)` call expression — it is genuine AST evidence, not invented.
            if matches!(assertion.oracle_kind, OracleKind::Unknown)
                && !assertion.matcher.is_empty()
                && !saw_custom_matcher
            {
                let sample = format!(
                    "{}:{}",
                    normalized_path(&candidate.test.file),
                    assertion.line
                );
                let why = format!(
                    "the test uses an unrecognised matcher `.{}(...)`; the adapter cannot classify its oracle strength without knowing the matcher's semantics",
                    assertion.matcher
                );
                limitations.push(TypeScriptNamedLimitation {
                    name: "typescript_custom_matcher_unresolved",
                    sample_source: sample,
                    why_not_actionable: why,
                    repair_route: "analysis/typescript-custom-matcher-resolution",
                });
                saw_custom_matcher = true;
            }
            // Dynamic assertion limitation: the matcher argument is a non-literal
            // dynamic expression (variable, function call, computed value).
            // Real producer: `oracle.rs::extract_matcher_expected_value` sets
            // `has_dynamic_matcher_arg = true` for such cases (RIPR-SPEC-0085 §PR5).
            if assertion.has_dynamic_matcher_arg && !saw_dynamic_assertion {
                if table_case_test(candidate.test) && !saw_table_case {
                    let sample = format!(
                        "{}:{}",
                        normalized_path(&candidate.test.file),
                        assertion.line
                    );
                    let why = format!(
                        "the table-driven test `{}` uses a row-derived dynamic value in `.{}(...)`; the adapter cannot statically bind table rows to concrete expected values",
                        candidate.test.name, assertion.matcher
                    );
                    limitations.push(TypeScriptNamedLimitation {
                        name: "typescript_table_case_unresolved",
                        sample_source: sample,
                        why_not_actionable: why,
                        repair_route: "analysis/typescript-table-case-resolution",
                    });
                    saw_table_case = true;
                }
                let sample = format!(
                    "{}:{}",
                    normalized_path(&candidate.test.file),
                    assertion.line
                );
                let why = format!(
                    "the matcher `.{}(...)` receives a non-literal dynamic argument; the adapter cannot statically resolve the expected value and cannot confirm the discriminator pins the changed behavior",
                    assertion.matcher
                );
                limitations.push(TypeScriptNamedLimitation {
                    name: "typescript_dynamic_assertion_unresolved",
                    sample_source: sample,
                    why_not_actionable: why,
                    repair_route: "analysis/typescript-dynamic-assertion-resolution",
                });
                saw_dynamic_assertion = true;
            }
        }
    }
    limitations
}

fn table_case_test(test: &TypeScriptTest) -> bool {
    let body = test.body_text.trim_start();
    body.starts_with("test.each(") || body.starts_with("it.each(")
}

fn oracle_helper_gated_call(
    owner: &TypeScriptOwner,
    test: &TypeScriptTest,
) -> Option<(String, usize)> {
    for (offset, line) in test.body_text.lines().enumerate() {
        if !contains_call_name(line, &owner.name) {
            continue;
        }
        let Some(helper_name) = assertion_helper_call_name(line, &owner.name) else {
            continue;
        };
        return Some((helper_name, test.line + offset));
    }
    None
}

fn assertion_helper_call_name(line: &str, owner_name: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") || trimmed.starts_with('*') {
        return None;
    }
    let trimmed = trimmed
        .strip_prefix("await ")
        .or_else(|| trimmed.strip_prefix("return "))
        .or_else(|| trimmed.strip_prefix("void "))
        .unwrap_or(trimmed)
        .trim_start();
    let before_args = trimmed.split_once('(')?.0.trim();
    if before_args.is_empty() || before_args.chars().any(char::is_whitespace) {
        return None;
    }
    let helper_name = before_args
        .rsplit(['.', ':'])
        .find(|part| !part.is_empty())?
        .trim();
    if helper_name == owner_name {
        return None;
    }
    let lower = helper_name.to_ascii_lowercase();
    let helper_shaped = lower.starts_with("assert") || lower.starts_with("expect");
    helper_shaped.then(|| helper_name.to_string())
}

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
