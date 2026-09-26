//! Related test candidate discovery for the TypeScript preview adapter.

use super::tsconfig::TsAliasMap;
use super::*;
use std::collections::{HashMap, HashSet};

// ── Re-export index ───────────────────────────────────────────────────────────

/// Single-hop re-export index built during Phase 1 of the adapter.
///
/// Maps `(intermediate_normalized_module, exported_name)` to
/// `(original_name, owner_normalized_module)`.
///
/// A test that imports `N` from intermediate file B is credited when:
/// 1. `(b_module, N)` resolves in the index to `(orig, owner_module)`, AND
/// 2. `owner_module` matches the normalized owner file, AND
/// 3. `orig` matches the owner function name.
///
/// Only ONE hop is followed; deeper transitive chains stay uncredited
/// (fail-closed). The index is empty when no re-exports are present, which
/// makes all callers that pass `ReExportIndex::empty()` behave identically
/// to the pre-fix behaviour.
#[derive(Debug, Default, Clone)]
pub(crate) struct ReExportIndex {
    /// key: (intermediate_module_norm, exported_name)
    /// value: (original_name, source_module_norm)
    entries: HashMap<(String, String), (String, String)>,
    /// `export * from './source'` hops: (intermediate_module, star_source_module).
    /// A star re-export forwards every named export of the source module under
    /// its original name, so a test importing `owner.name` from `intermediate`
    /// reaches the owner when a star hop leads to the owner's module
    /// (#4103 under-credit: the star barrel was invisible and the emitted
    /// guidance claimed no test referenced the owner at all).
    star_edges: HashSet<(String, String)>,
}

impl ReExportIndex {
    /// Construct an empty index (no re-export tracing).
    /// Used by unit-test callers that do not exercise the re-export path.
    ///
    /// Test-only: production callers (the live pipeline and the mock-path
    /// collector) always thread the real index through, so an empty index is
    /// never constructed outside `cfg(test)`.
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    /// Test-only constructor from raw entries and star edges, so re-export
    /// resolution can be exercised without a real filesystem.
    #[cfg(test)]
    pub(crate) fn from_parts(
        entries: Vec<((String, String), (String, String))>,
        star_edges: Vec<(String, String)>,
    ) -> Self {
        Self {
            entries: entries.into_iter().collect(),
            star_edges: star_edges.into_iter().collect(),
        }
    }

    /// Build a re-export index from all non-test source files in the workspace.
    ///
    /// For each file that contains `export { N [as M] } from './A'` statements,
    /// records the single hop from `(intermediate_file, M)` → `(N, A_module)`.
    /// Only explicit named re-exports from relative paths are indexed;
    /// star-re-exports (`export * from`) and non-relative sources are ignored
    /// (fail-closed).
    ///
    /// `sources` is the Phase-1 workspace source cache: every file is read at
    /// most once per analysis run. Files absent from the cache (unreadable or
    /// over the read caps) are skipped here; their limitation disclosure is
    /// owned by the read pipeline.
    ///
    /// `alias_map` is forwarded to `normalized_relative_import_module` so that
    /// tsconfig.json-aliased sources (e.g. `@/owner`) can be followed through
    /// re-exports when `resolve_tsconfig_paths` is enabled.
    pub(crate) fn build(
        workspace_files: &[PathBuf],
        sources: &HashMap<PathBuf, String>,
        workspace_root: &Path,
        alias_map: Option<&TsAliasMap>,
        is_test: impl Fn(&Path) -> bool,
    ) -> Self {
        use oxc_parser::Parser;

        let mut entries: HashMap<(String, String), (String, String)> = HashMap::new();
        let mut star_edges: HashSet<(String, String)> = HashSet::new();
        for relative in workspace_files {
            if is_test(relative) {
                continue;
            }
            let Some(source) = sources.get(relative) else {
                continue;
            };
            // Parse on the guarded large-stack worker (#4101): the recursive
            // oxc parser must not run on the caller's stack, and an
            // over-budget source is skipped exactly like a parse error.
            let worker_root = workspace_root.to_path_buf();
            let worker_alias = alias_map.cloned();
            let Ok(file_facts) = parse_on_worker(
                relative.as_path(),
                source.as_str(),
                move |relative, source, allocator| {
                    let ret = Parser::new(allocator, source, source_type_for(relative)).parse();
                    if !ret.errors.is_empty() {
                        return (Vec::new(), Vec::new());
                    }
                    // intermediate module path (normalized, no extension)
                    let intermediate_module = normalized_module_path(relative);
                    let mut file_entries = Vec::new();
                    let mut file_star_edges = Vec::new();
                    for stmt in &ret.program.body {
                        if let Statement::ExportAllDeclaration(export_all) = stmt {
                            // `export * from './owner'` — record the star hop.
                            // `export * as ns from './owner'` namespaces the
                            // module value, so the named-forwarding contract a
                            // star hop carries does not apply; it stays
                            // unindexed (fail-closed).
                            if export_all.exported.is_some() {
                                continue;
                            }
                            let star_source = export_all.source.value.to_string();
                            if let Some(resolved) = normalized_relative_import_module(
                                relative,
                                &star_source,
                                worker_alias.as_ref(),
                                Some(worker_root.as_path()),
                            ) {
                                file_star_edges.push((intermediate_module.clone(), resolved));
                            }
                            continue;
                        }
                        let Statement::ExportNamedDeclaration(export) = stmt else {
                            continue;
                        };
                        if export.declaration.is_some() {
                            continue;
                        }
                        let Some(re_source) = &export.source else {
                            // Owner-file RENAME export (#4104-B):
                            // `export { computeTotals as totals }` with no
                            // `from` re-binds the file's OWN local binding
                            // under `exported_name`. Record the self-hop so
                            // `import { totals } from './owner-file'`
                            // resolves back to the original owner name.
                            for specifier in &export.specifiers {
                                if specifier.export_kind == ImportOrExportKind::Type {
                                    continue;
                                }
                                let Some(local_name) = module_export_name_text(&specifier.local)
                                else {
                                    continue;
                                };
                                let exported_name = module_export_name_text(&specifier.exported)
                                    .unwrap_or_else(|| local_name.clone());
                                if exported_name == local_name {
                                    // Plain re-export of the same name needs no index.
                                    continue;
                                }
                                let key = (intermediate_module.clone(), exported_name);
                                file_entries.push((key, (local_name, intermediate_module.clone())));
                            }
                            continue;
                        };
                        let source_str = re_source.value.to_string();
                        // Resolve the source module relative to the intermediate file's dir.
                        // Pass alias_map so tsconfig-aliased paths can be followed.
                        let Some(resolved) = normalized_relative_import_module(
                            relative,
                            &source_str,
                            worker_alias.as_ref(),
                            Some(worker_root.as_path()),
                        ) else {
                            continue;
                        };
                        for specifier in &export.specifiers {
                            if specifier.export_kind == ImportOrExportKind::Type {
                                continue;
                            }
                            let Some(original_name) = module_export_name_text(&specifier.local)
                            else {
                                continue;
                            };
                            let exported_name = module_export_name_text(&specifier.exported)
                                .unwrap_or_else(|| original_name.clone());
                            // key: what the test would import from the intermediate file
                            let key = (intermediate_module.clone(), exported_name);
                            // value: what the owner file exports under its original name
                            file_entries.push((key, (original_name, resolved.clone())));
                        }
                    }
                    (file_entries, file_star_edges)
                },
            ) else {
                continue;
            };
            let (file_entries, file_star_edges) = file_facts;
            for (key, value) in file_entries {
                entries.entry(key).or_insert_with(|| value);
            }
            star_edges.extend(file_star_edges);
        }
        Self {
            entries,
            star_edges,
        }
    }

    /// If `test_file` imports `imported_name` from `intermediate_module` and
    /// the index resolves that to the owner, return the original name in the
    /// owner file.  Returns `None` when no single-hop chain leads to the owner.
    fn resolve_to_owner(
        &self,
        test_file: &Path,
        import_source: &str,
        imported_name: &str,
        owner: &TypeScriptOwner,
        alias_map: Option<&TsAliasMap>,
        workspace_root: Option<&Path>,
    ) -> bool {
        // Resolve the import source to a normalized module path.
        let Some(intermediate_module) =
            normalized_relative_import_module(test_file, import_source, alias_map, workspace_root)
        else {
            return false;
        };
        let owner_module = normalized_module_path(&owner.file);
        // The import targets the owner module directly. A plain same-name
        // import is handled by the other relations; only an owner-file
        // RENAME export (`export { ownerName as alias }`) needs the index
        // here, because the test imports `alias`, not the owner name
        // (#4104-B).
        if intermediate_module == owner_module {
            let key = (intermediate_module, imported_name.to_string());
            let Some((original_name, source_module)) = self.entries.get(&key) else {
                return false;
            };
            return source_module == &owner_module && original_name == &owner.name;
        }
        let key = (intermediate_module.clone(), imported_name.to_string());
        let Some((original_name, source_module)) = self.entries.get(&key) else {
            // Star-hop fallback (#4103): `export * from './owner'` forwards
            // every named export under its original name, so importing
            // `owner.name` from a barrel that star-exports the owner's module
            // reaches the owner. The imported name must equal the owner's own
            // name — a star hop invents no aliases.
            return imported_name == owner.name
                && self
                    .star_edges
                    .contains(&(intermediate_module, owner_module.clone()));
        };
        // The chain must resolve to the owner's file and the owner's name.
        // `export { default as X } from './owner'` records the original name
        // `default`; it resolves to THIS owner exactly when the owner is the
        // owner module's default export (#4103 under-credit: that chain was
        // silently indexed but could never match a named owner).
        source_module == &owner_module
            && (original_name == &owner.name
                || (original_name == "default" && owner.exported_as_default))
    }
}

/// Resolve the nearest `package.json` root for `file` by walking up to
/// `workspace_root`.  Returns the package root path (relative to
/// `workspace_root`) when found.
///
/// This is a lightweight path-only walk — it does NOT parse the manifest and
/// does NOT perform full `PackageDiscovery`. It is used only for the
/// package-local ownership filter.
pub(crate) fn package_root_for_file_path(file: &Path, workspace_root: &Path) -> Option<PathBuf> {
    let absolute_file = if file.is_absolute() {
        file.to_path_buf()
    } else {
        workspace_root.join(file)
    };
    let start = absolute_file.parent()?;
    let mut current = start.to_path_buf();
    loop {
        if current.join("package.json").is_file() {
            // Convert back to workspace-relative.
            return current.strip_prefix(workspace_root).ok().map(|rel| {
                if rel == Path::new("") {
                    PathBuf::from(".")
                } else {
                    rel.to_path_buf()
                }
            });
        }
        if current == workspace_root {
            break;
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }
    // Check workspace root itself.
    if workspace_root.join("package.json").is_file() {
        return Some(PathBuf::from("."));
    }
    None
}

/// Return `true` when `owner_file` and `test_file` both resolve to the same
/// package root under `workspace_root`, or when either file's package root
/// cannot be determined (fail-open: preserve existing behaviour when no
/// `package.json` hierarchy exists).
///
/// A cross-package candidate is one where the owner lives in
/// `packages/a/` and the test lives in `packages/b/`.  Such a candidate
/// MUST NOT be selected as an owned relation.
pub(crate) fn same_package_root(
    owner_file: &Path,
    test_file: &Path,
    workspace_root: &Path,
) -> bool {
    let owner_pkg = package_root_for_file_path(owner_file, workspace_root);
    let test_pkg = package_root_for_file_path(test_file, workspace_root);
    match (owner_pkg, test_pkg) {
        // Both found: they must agree.
        (Some(a), Some(b)) => a == b,
        // If either is unresolved, fail-open (preserve existing behaviour).
        _ => true,
    }
}

/// Collect related test candidates for `owner` from `all_tests`.
///
/// `workspace_root` enables package-local ownership filtering when `Some`:
/// tests in different packages are excluded from the candidate set so that a
/// test in `packages/b/` cannot be selected as an owner relation for a source
/// file in `packages/a/`.  Pass `None` to preserve the previous behaviour
/// (used in unit tests that do not have a real filesystem).
///
/// `reexport_index` enables single-hop re-export tracing: tests that import
/// the owner through an intermediate barrel file are credited when the chain
/// can be resolved in-source in a single hop.  Pass `&ReExportIndex::empty()`
/// to disable re-export tracing (backward-compatible default for unit tests).
///
/// `alias_map` enables tsconfig.json path alias resolution for non-relative
/// specifiers.  Pass `None` when `resolve_tsconfig_paths` is `false` (default).
pub(crate) fn related_test_candidates<'a>(
    owner: &TypeScriptOwner,
    all_tests: &'a [TypeScriptTest],
    workspace_root: Option<&Path>,
    reexport_index: &ReExportIndex,
    alias_map: Option<&TsAliasMap>,
) -> Vec<TypeScriptRelatedCandidate<'a>> {
    let mut candidates: Vec<TypeScriptRelatedCandidate<'a>> = all_tests
        .iter()
        .filter(|test| {
            workspace_root
                .map(|root| same_package_root(&owner.file, &test.file, root))
                .unwrap_or(true)
        })
        .filter_map(|test| {
            owner_call_relation(test, owner, reexport_index, alias_map, workspace_root)
                .map(|relation| TypeScriptRelatedCandidate { test, relation })
        })
        .collect();
    if candidates.is_empty() {
        candidates = all_tests
            .iter()
            .filter(|test| {
                workspace_root
                    .map(|root| same_package_root(&owner.file, &test.file, root))
                    .unwrap_or(true)
            })
            .filter_map(|test| {
                heuristic_relation(test, owner, alias_map, workspace_root)
                    .map(|relation| TypeScriptRelatedCandidate { test, relation })
            })
            .collect();
    }
    sort_related_candidates(&mut candidates);
    candidates
}

pub(crate) fn sort_related_candidates(candidates: &mut [TypeScriptRelatedCandidate<'_>]) {
    candidates.sort_by(|left, right| {
        right
            .relation
            .rank()
            .cmp(&left.relation.rank())
            .then_with(|| {
                let left_rank = strongest_assertion(&left.test.assertions)
                    .map(|assertion| assertion.oracle_strength.rank())
                    .unwrap_or(0);
                let right_rank = strongest_assertion(&right.test.assertions)
                    .map(|assertion| assertion.oracle_strength.rank())
                    .unwrap_or(0);
                right_rank.cmp(&left_rank)
            })
            .then_with(|| left.test.file.cmp(&right.test.file))
            .then_with(|| left.test.line.cmp(&right.test.line))
            .then_with(|| left.test.name.cmp(&right.test.name))
    });
}

pub(crate) fn owner_call_relation(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    reexport_index: &ReExportIndex,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> Option<TypeScriptRelationKind> {
    if owner.owner_kind == OwnerKind::ModuleFunction {
        return module_initializer_observer_relation(test, owner, alias_map, workspace_root)
            .then_some(TypeScriptRelationKind::ModuleValueReference);
    }
    if owner.owner_kind == OwnerKind::Method {
        return receiver_owner_call_relation(test, owner, alias_map, workspace_root)
            .then_some(TypeScriptRelationKind::ReceiverOwnerCall);
    }
    if owner.owner_kind == OwnerKind::ClassMethod {
        return class_method_owner_call_relation(test, owner, alias_map, workspace_root)
            .then_some(TypeScriptRelationKind::ClassMethodCall);
    }
    // Owner-module mock guard (issue #2269): when the test mocks the changed
    // owner's own module, every owner call below executes the mock, not the
    // changed code, so no owner-call relation may be credited — the same
    // guard the Method/ClassMethod/ModuleFunction paths above already apply.
    // The heuristic fallback may still link the test as advisory proximity
    // evidence (fail-closed: at most `weakly_exposed`).
    if test_mocks_owner_module(test, owner, alias_map, workspace_root) {
        return None;
    }
    if contains_call_name(&test.body_text, &owner.name)
        && !owner_name_shadowed_by_unrelated_import(test, owner, alias_map, workspace_root)
        // Shadow guard (#4102): a test body that declares its own
        // `function <owner>(...)` / `const <owner> = ...` calls the local
        // declaration, not the imported owner — do not credit DirectOwnerCall.
        && !local_identifier_declared_in_test_body(&test.body_text, &owner.name)
    {
        return Some(TypeScriptRelationKind::DirectOwnerCall);
    }
    // Non-namespace alias-rename: `import { OriginalName as local }` from the
    // owner file and `local(...)` in the test body.  Shadow guard: if `local`
    // is re-declared inside the test body the `cv(...)` call reaches the shadow,
    // not the owner — do NOT credit `ImportAliasOwnerCall` in that case.
    if let Some(_import) = test.imports_in_file.iter().find(|import| {
        !import.namespace
            && import_source_matches_owner(import, &test.file, owner, alias_map, workspace_root)
            && import.imported.as_deref() == Some(owner.name.as_str())
            && import.local != owner.name
            && contains_call_name(&test.body_text, &import.local)
            && !local_identifier_declared_in_test_body(&test.body_text, &import.local)
    }) {
        return Some(TypeScriptRelationKind::ImportAliasOwnerCall);
    }
    if test.imports_in_file.iter().any(|import| {
        import_source_matches_owner(import, &test.file, owner, alias_map, workspace_root)
            && import_references_owner_call(import, &test.body_text, owner)
    }) {
        return Some(TypeScriptRelationKind::ImportedOwnerCall);
    }
    // Dynamic `const m = await import("./owner")` member access (#4103
    // under-credit): the import expression produces no static import record,
    // so the namespace arm above cannot see it. When the test body itself
    // binds the awaited import to a local and calls the owner through that
    // local, the resolved specifier anchors the same identity the require()
    // namespace form already credits.
    // Default-import credit (`import ride from './owner'` + `ride(...)`,
    // #4104-B) is owned by `import_references_owner_call` above via
    // `owner.exported_as_default` (landed with #4137); a separate arm here
    // would duplicate it.
    if dynamic_import_member_call(test, owner, alias_map, workspace_root) {
        return Some(TypeScriptRelationKind::ImportedOwnerCall);
    }
    // Single-hop re-export tracing (RIPR-SPEC-0095):
    // If the test imports a name from an intermediate file that re-exports it
    // from the owner file, credit the test via re_export_chain_followed.
    // Only one hop is followed; deeper chains stay uncredited (fail-closed).
    // The index also carries the owner file's OWN rename exports
    // (`export { ownerName as alias }`), so `import { alias } from
    // './owner-file'` resolves back to the owner name (#4104-B).
    if test.imports_in_file.iter().any(|import| {
        if import.namespace {
            return false; // namespace imports don't map cleanly to a single exported name
        }
        let Some(imported_name) = import.imported.as_deref() else {
            return false;
        };
        if imported_name == "default" {
            return false; // default imports are out of scope for single-hop re-export
        }
        // The local alias in the test is what gets called; check the call site.
        let local = &import.local;
        if !contains_call_name(&test.body_text, local) {
            return false;
        }
        reexport_index.resolve_to_owner(
            &test.file,
            &import.source,
            imported_name,
            owner,
            alias_map,
            workspace_root,
        )
    }) {
        return Some(TypeScriptRelationKind::ReExportChainFollowed);
    }
    None
}

pub(crate) fn module_initializer_observer_relation(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    if owner.owner_kind != OwnerKind::ModuleFunction
        || test_mocks_owner_module(test, owner, alias_map, workspace_root)
    {
        return false;
    }
    if normalized_module_path(&test.file) == normalized_module_path(&owner.file)
        && !local_identifier_declared_in_test_body(&test.body_text, &owner.name)
        && expect_actual_references_identifier(&test.body_text, &owner.name)
    {
        return true;
    }
    test.imports_in_file.iter().any(|import| {
        if !import_source_matches_owner(import, &test.file, owner, alias_map, workspace_root) {
            return false;
        }
        if import.namespace {
            return expect_actual_references_member(&test.body_text, &import.local, &owner.name);
        }
        import.imported.as_deref() == Some(owner.name.as_str())
            && !local_identifier_declared_in_test_body(&test.body_text, &import.local)
            && expect_actual_references_identifier(&test.body_text, &import.local)
    })
}

pub(crate) fn receiver_owner_call_relation(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    if owner.owner_kind != OwnerKind::Method
        || test_mocks_owner_module(test, owner, alias_map, workspace_root)
    {
        return false;
    }
    // Constructor owner (#4104-B): the changed constructor body runs on EVERY
    // construction, so a test that executes `new ClassName(...)` exercises it
    // directly — there is no member-call needle for a `constructor` name.
    if owner.method_kind == TypeScriptMethodKind::Constructor {
        let mut constructor_names = Vec::new();
        if let Some(class_name) = owner.class_name.as_deref() {
            constructor_names.push(class_name.to_string());
        }
        // Default-import alias (review #4138): `import Bar from './cart'` +
        // `new Bar(...)` runs the constructor of `export default class Cart`
        // even though the class name never appears. The alias candidate is
        // resolved through the test's own default import of the owner module
        // and only when the owner's class IS that module's default export
        // (`class_default_export`); a locally re-declared alias constructs
        // the shadow, not the owner.
        if owner.class_default_export {
            for import in &test.imports_in_file {
                if !import.namespace
                    && import.imported.as_deref() == Some("default")
                    && import_source_matches_owner(
                        import,
                        &test.file,
                        owner,
                        alias_map,
                        workspace_root,
                    )
                    && !local_identifier_declared_in_test_body(&test.body_text, &import.local)
                {
                    constructor_names.push(import.local.clone());
                }
            }
        }
        return constructor_names
            .iter()
            .any(|candidate| contains_new_expression_call(&test.body_text, candidate));
    }
    let constructor_names =
        constructor_names_for_method_owner(test, owner, alias_map, workspace_root);
    let receiver_names = receiver_names_for_constructor_calls(&test.body_text, &constructor_names);
    if receiver_names
        .iter()
        .any(|receiver| contains_member_call_name(&test.body_text, receiver, &owner.name))
    {
        return true;
    }
    // Getter owner (#4104-B): a getter is INVOKED by reading the property on
    // a constructed receiver (`expect(cart.total).toBe(80)` runs the `get
    // total()` body), so a member READ on the receiver is the honest call
    // needle. Ordinary methods keep the call-only requirement: a bare
    // property read does not execute them.
    if owner.method_kind == TypeScriptMethodKind::Getter {
        return receiver_names
            .iter()
            .any(|receiver| contains_member_read_name(&test.body_text, receiver, &owner.name));
    }
    false
}

pub(crate) fn class_method_owner_call_relation(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    if owner.owner_kind != OwnerKind::ClassMethod
        || test_mocks_owner_module(test, owner, alias_map, workspace_root)
    {
        return false;
    }
    let class_names = class_names_for_class_method_owner(test, owner, alias_map, workspace_root);
    if class_names.is_empty() {
        return false;
    }
    class_names
        .iter()
        .any(|class_name| contains_member_call_name(&test.body_text, class_name, &owner.name))
}

pub(crate) fn class_names_for_class_method_owner(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> Vec<String> {
    let Some(class_name) = owner.class_name.as_deref() else {
        return Vec::new();
    };
    let mut names = Vec::new();
    if normalized_module_path(&test.file) == normalized_module_path(&owner.file)
        && !local_identifier_declared_in_test_body(&test.body_text, class_name)
    {
        push_unique_string(&mut names, class_name.to_string());
    }
    for import in &test.imports_in_file {
        if import.namespace
            || !import_source_matches_owner(import, &test.file, owner, alias_map, workspace_root)
        {
            continue;
        }
        if import.imported.as_deref() == Some(class_name)
            && !local_identifier_declared_in_test_body(&test.body_text, &import.local)
        {
            push_unique_string(&mut names, import.local.clone());
        }
    }
    names
}

pub(crate) fn constructor_names_for_method_owner(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> Vec<String> {
    let Some(class_name) = owner.class_name.as_deref() else {
        return Vec::new();
    };
    let mut names = Vec::new();
    if normalized_module_path(&test.file) == normalized_module_path(&owner.file) {
        push_unique_string(&mut names, class_name.to_string());
    }
    for import in &test.imports_in_file {
        if import.namespace
            || !import_source_matches_owner(import, &test.file, owner, alias_map, workspace_root)
        {
            continue;
        }
        if import.imported.as_deref() == Some(class_name) {
            push_unique_string(&mut names, import.local.clone());
        }
    }
    names
}

pub(crate) fn receiver_names_for_constructor_calls(
    body_text: &str,
    constructor_names: &[String],
) -> Vec<String> {
    let mut receiver_names = Vec::new();
    for line in body_text.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("//") {
            continue;
        }
        let Some(after_keyword) = ["const ", "let ", "var "]
            .into_iter()
            .find_map(|keyword| trimmed.strip_prefix(keyword))
        else {
            continue;
        };
        let Some((declaration, initializer)) = after_keyword.split_once('=') else {
            continue;
        };
        let Some(receiver_name) = receiver_name_from_declaration(declaration) else {
            continue;
        };
        if constructor_names
            .iter()
            .any(|constructor| contains_new_constructor_call(initializer, constructor))
        {
            push_unique_string(&mut receiver_names, receiver_name);
        }
    }
    receiver_names
}

pub(crate) fn receiver_name_from_declaration(declaration: &str) -> Option<String> {
    if declaration.contains(',') {
        return None;
    }
    let name = declaration.split(':').next()?.trim();
    is_safe_javascript_identifier(name).then(|| name.to_string())
}

pub(crate) fn contains_new_constructor_call(text: &str, constructor_name: &str) -> bool {
    let needle = format!("new {constructor_name}(");
    text.match_indices(&needle).any(|(idx, _)| {
        text[..idx]
            .chars()
            .next_back()
            .is_none_or(|ch| !is_javascript_identifier_char(ch) && ch != '.')
            && !line_prefix_looks_like_comment_or_string(text, idx)
            && !inside_block_comment(text, idx)
    })
}

fn test_mocks_owner_module(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    test.mocks_in_file.iter().any(|source| {
        normalized_relative_import_module(&test.file, source, alias_map, workspace_root)
            .is_some_and(|module| module == normalized_module_path(&owner.file))
    })
}

/// `const m = await import("../src/pricing");` followed by an
/// `m.loyaltyPrice(...)` member call, both inside the test body (#4103
/// under-credit).
///
/// The dynamic import expression produces no static import record, so this
/// scans the test body directly: every `import("...")` specifier is resolved
/// like a static import, and when it resolves to the owner's own module the
/// test must also declare the awaited binding with the simple
/// `const/let/var X = await import("...")` declarator form and call the owner
/// through that local. Anything else — `(await import(...)).default`,
/// `.then(m => ...)` callbacks, destructured dynamic bindings — stays
/// fail-closed (the binding cannot be attributed this narrowly).
fn dynamic_import_member_call(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    let body = &test.body_text;
    let owner_module = normalized_module_path(&owner.file);
    let mut from = 0usize;
    while let Some(rel) = body[from..].find("import(") {
        let at = from + rel;
        from = at + "import(".len();
        // Not a dynamic import: `myimport(` or a member `x.import(`.
        if body[..at]
            .chars()
            .next_back()
            .is_some_and(is_javascript_identifier_char)
        {
            continue;
        }
        if line_prefix_looks_like_comment_or_string(body, at) || inside_block_comment(body, at) {
            continue;
        }
        let Some(specifier) = quoted_literal_after(body, from) else {
            continue;
        };
        let resolves_to_owner =
            normalized_relative_import_module(&test.file, &specifier, alias_map, workspace_root)
                .is_some_and(|module| module == owner_module);
        if !resolves_to_owner {
            continue;
        }
        let Some(binding) = awaited_import_binding_before(body, at) else {
            continue;
        };
        if contains_member_call_name(body, &binding, &owner.name) {
            return true;
        }
    }
    false
}

/// The quoted literal (single, double, or backtick) starting at `from`,
/// allowing whitespace between the paren and the quote (`import( "./x" )`).
fn quoted_literal_after(body: &str, from: usize) -> Option<String> {
    let rest = body.get(from..)?.trim_start();
    let quote = rest.chars().next()?;
    if !matches!(quote, '"' | '\'' | '`') {
        return None;
    }
    let payload = rest.get(quote.len_utf8()..)?;
    let end = payload.find(quote)?;
    Some(payload[..end].to_string())
}

/// Recover the local binding of `const X = await import(...)` from the text
/// before the import call. Requires the import call to be the declarator's
/// whole initializer on its line (optionally wrapped in `await`); a trailing
/// member access like `.default` fails the scan (fail-closed).
fn awaited_import_binding_before(body: &str, at: usize) -> Option<String> {
    let line_start = body[..at].rfind('\n').map_or(0, |offset| offset + 1);
    let trimmed = body[line_start..at].trim();
    // The import must actually be awaited: `const m = import(...)` binds a
    // promise, so a member call through `m` cannot reach the owner.
    let after_await = trimmed.strip_suffix("await")?.trim_end();
    let (head, tail) = after_await.rsplit_once('=')?;
    if !tail.is_empty() {
        return None;
    }
    let head = head.trim_end();
    let binding = ["const ", "let ", "var "]
        .into_iter()
        .find_map(|keyword| head.strip_prefix(keyword))?
        .trim();
    // Strip a type annotation: `const m: Pricing = await import(...)`.
    let name = binding.split(':').next()?.trim();
    is_safe_javascript_identifier(name).then(|| name.to_string())
}

fn heuristic_relation(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> Option<TypeScriptRelationKind> {
    if !heuristic_owner_supported(owner) {
        return None;
    }
    if !heuristic_relation_allowed(test, owner, alias_map, workspace_root) {
        return None;
    }
    // A test is related to an owner only when it references the owner
    // (RIPR-SPEC-0027). File-stem proximity and describe/test names are ranking
    // labels for a test that already references the owner without a recognized
    // call shape; they never relate a test that only exercises a sibling owner
    // in the same module (F5-9: `loyaltyPrice` was linked to tests that only
    // call `discountedTotal`).
    if !test_references_owner(test, owner, alias_map, workspace_root) {
        return None;
    }
    if same_file_proximity_related(test, owner) {
        return Some(TypeScriptRelationKind::SameFileProximity);
    }
    if describe_name_similar_to_owner(test, owner) {
        return Some(TypeScriptRelationKind::DescribeName);
    }
    if test_name_similar_to_owner(test, owner) {
        return Some(TypeScriptRelationKind::TestName);
    }
    None
}

/// Whether the test body references the changed owner by name, even when the
/// reference is not a recognized direct call.
///
/// Counted references, each outside comments and string literals:
///
/// - the bare owner identifier (`expect(loyaltyPrice).toBeDefined()`,
///   `const fn = loyaltyPrice`, `<Badge />`), unless the test body declares a
///   local of the same name or the identifier is an object-literal key;
/// - an import-renamed local (`import { loyaltyPrice as lp }`) or a namespace
///   member (`pricing.loyaltyPrice`) whose import resolves to the owner module.
///
/// A test that names the owner only in its title, its `describe(...)` title, or
/// its file stem does not reference the owner, and neither does an arbitrary
/// object-member use such as `order.loyaltyPrice(...)`, which the direct-call
/// rule already refuses to relate.
fn test_references_owner(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    let body = &test.body_text;
    if !local_identifier_declared_in_test_body(body, &owner.name)
        && contains_identifier_reference(body, &owner.name)
    {
        return true;
    }
    test.imports_in_file.iter().any(|import| {
        if !import_source_matches_owner(import, &test.file, owner, alias_map, workspace_root) {
            return false;
        }
        if import.namespace {
            return contains_member_reference(body, &import.local, &owner.name);
        }
        // Same default-export rule as `import_references_owner_call` (#4103):
        // a default import binds the owner only when the owner is the
        // module's default export.
        import.imported.as_deref().is_some_and(|imported| {
            imported == owner.name || (imported == "default" && owner.exported_as_default)
        }) && !local_identifier_declared_in_test_body(body, &import.local)
            && contains_identifier_reference(body, &import.local)
    })
}

/// Bare identifier reference: identifier boundaries on both sides, not a
/// member property (`x.name`), not an object-literal key (`{ name: 1 }`), and
/// not inside a comment or string literal. Boundaries use the ASCII
/// identifier set of `is_javascript_identifier_char`, so a non-ASCII
/// identifier character next to the name counts as a boundary.
fn contains_identifier_reference(body_text: &str, identifier: &str) -> bool {
    if !is_safe_javascript_identifier(identifier) {
        return false;
    }
    body_text.match_indices(identifier).any(|(idx, _)| {
        let end = idx + identifier.len();
        has_member_call_boundary(body_text, idx)
            && body_text[end..]
                .chars()
                .next()
                .is_none_or(|ch| !is_javascript_identifier_char(ch))
            && !is_object_literal_key(body_text, idx, end)
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
            && !inside_block_comment(body_text, idx)
    })
}

/// `object.property` with identifier boundaries, outside comments and strings.
/// Plain, optional-chain (`obj?.p`) and non-null (`obj!.p`) accesses all reach
/// the same member of the same object binding (#4103 under-credit).
fn contains_member_reference(body_text: &str, object_name: &str, property_name: &str) -> bool {
    if !is_safe_javascript_identifier(object_name) || !is_safe_javascript_identifier(property_name)
    {
        return false;
    }
    [
        format!("{object_name}.{property_name}"),
        format!("{object_name}?.{property_name}"),
        format!("{object_name}!.{property_name}"),
    ]
    .iter()
    .any(|needle| {
        body_text.match_indices(needle.as_str()).any(|(idx, _)| {
            has_member_call_boundary(body_text, idx)
                && body_text[idx + needle.len()..]
                    .chars()
                    .next()
                    .is_none_or(|ch| !is_javascript_identifier_char(ch))
                && !line_prefix_looks_like_comment_or_string(body_text, idx)
                && !inside_block_comment(body_text, idx)
        })
    })
}

fn is_object_literal_key(body_text: &str, start: usize, end: usize) -> bool {
    let next_is_colon = body_text[end..]
        .trim_start()
        .strip_prefix(':')
        .is_some_and(|rest| !rest.starts_with(':'));
    let previous_opens_entry = body_text[..start]
        .trim_end()
        .chars()
        .next_back()
        .is_some_and(|ch| ch == '{' || ch == ',');
    next_is_colon && previous_opens_entry
}

fn heuristic_owner_supported(owner: &TypeScriptOwner) -> bool {
    matches!(
        owner.owner_kind,
        OwnerKind::Function | OwnerKind::ArrowFunction | OwnerKind::Component
    )
}

/// Find related tests for `owner` in `all_tests`.
///
/// `workspace_root` enables package-local ownership filtering when `Some`:
/// tests in different packages are excluded from the candidate set.
/// Pass `None` to preserve the previous behaviour (used in unit tests).
///
/// `reexport_index` enables single-hop re-export tracing.
/// Pass `&ReExportIndex::empty()` to disable (backward-compatible default).
///
/// `alias_map` enables tsconfig.json path alias resolution for non-relative
/// specifiers.  Pass `None` when `resolve_tsconfig_paths` is `false` (default).
pub(crate) fn find_related_tests(
    owner: &TypeScriptOwner,
    all_tests: &[TypeScriptTest],
    workspace_root: Option<&Path>,
    reexport_index: &ReExportIndex,
    alias_map: Option<&TsAliasMap>,
) -> Vec<RelatedTest> {
    related_test_candidates(owner, all_tests, workspace_root, reexport_index, alias_map)
        .into_iter()
        .map(|candidate| {
            let strongest = candidate
                .relation
                .uses_oracle()
                .then(|| strongest_assertion(&candidate.test.assertions))
                .flatten();
            let (oracle_kind, oracle_strength, oracle_text) = match strongest {
                Some(assertion) => (
                    assertion.oracle_kind.clone(),
                    assertion.oracle_strength.clone(),
                    Some(assertion_oracle_text(assertion)),
                ),
                None => (OracleKind::Unknown, OracleStrength::Unknown, None),
            };
            // Map TypeScriptRelationKind to domain RelationReason for disclosure.
            let (relation_reason, relation_confidence) = ts_relation_to_domain(candidate.relation);
            RelatedTest {
                name: candidate.test.name.clone(),
                file: candidate.test.file.clone(),
                line: candidate.test.line,
                oracle: oracle_text,
                oracle_kind,
                oracle_strength,
                relation_reason,
                relation_confidence,
            }
        })
        .collect()
}

/// Map a `TypeScriptRelationKind` to the domain `(RelationReason, RelationConfidence)`.
///
/// Populates the `relation_reason` and `relation_confidence` fields of
/// `RelatedTest` for disclosure in the JSON output.  Returns `(None, None)`
/// for heuristic relations that don't have a clear domain mapping (those stay
/// legacy-style with `relation_reason: null`).
fn ts_relation_to_domain(
    kind: TypeScriptRelationKind,
) -> (
    Option<crate::domain::RelationReason>,
    Option<crate::domain::RelationConfidence>,
) {
    use crate::domain::{RelationConfidence, RelationReason};
    let reason = match kind {
        TypeScriptRelationKind::DirectOwnerCall => RelationReason::DirectOwnerCall,
        // Alias-rename import (`{ X as local }`) + verified body call: the
        // rename is syntactically explicit and the call is to the local alias —
        // same certainty as DirectOwnerCall.
        TypeScriptRelationKind::ImportAliasOwnerCall => RelationReason::DirectOwnerCall,
        TypeScriptRelationKind::ImportedOwnerCall => RelationReason::ImportPathAffinity,
        TypeScriptRelationKind::ModuleValueReference => RelationReason::DirectOwnerCall,
        TypeScriptRelationKind::ReceiverOwnerCall => RelationReason::DirectOwnerCall,
        TypeScriptRelationKind::ClassMethodCall => RelationReason::DirectOwnerCall,
        TypeScriptRelationKind::ReExportChainFollowed => RelationReason::ReExportChainFollowed,
        // Heuristic relations: no strong domain mapping — emit None to preserve
        // the existing behaviour for these lower-confidence relation kinds.
        TypeScriptRelationKind::SameFileProximity
        | TypeScriptRelationKind::DescribeName
        | TypeScriptRelationKind::TestName => return (None, None),
    };
    let confidence = match kind {
        TypeScriptRelationKind::DirectOwnerCall
        | TypeScriptRelationKind::ImportAliasOwnerCall
        | TypeScriptRelationKind::ModuleValueReference
        | TypeScriptRelationKind::ReceiverOwnerCall
        | TypeScriptRelationKind::ClassMethodCall => RelationConfidence::High,
        TypeScriptRelationKind::ImportedOwnerCall
        | TypeScriptRelationKind::ReExportChainFollowed => RelationConfidence::Medium,
        TypeScriptRelationKind::SameFileProximity
        | TypeScriptRelationKind::DescribeName
        | TypeScriptRelationKind::TestName => RelationConfidence::Low,
    };
    (Some(reason), Some(confidence))
}

fn heuristic_relation_allowed(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    !owner_name_shadowed_by_unrelated_import(test, owner, alias_map, workspace_root)
        && !owner_export_imported_from_unrelated_source(test, owner, alias_map, workspace_root)
}

pub(crate) fn contains_call_name(body_text: &str, call_name: &str) -> bool {
    // `name(` and the TypeScript optional-call form `name?.(...)` are the same
    // call of the same binding (#4103 under-credit: the `?.` form was
    // invisible to every call arm).
    [format!("{call_name}("), format!("{call_name}?.(")]
        .iter()
        .any(|needle| {
            body_text.match_indices(needle.as_str()).any(|(idx, _)| {
                has_call_boundary(body_text, idx)
                    && !line_prefix_looks_like_comment_or_string(body_text, idx)
                    && !inside_block_comment(body_text, idx)
            })
        })
}

fn has_call_boundary(body_text: &str, idx: usize) -> bool {
    let prefix = &body_text[..idx];
    let Some(ch) = prefix.chars().next_back() else {
        return true;
    };
    if is_javascript_identifier_char(ch) {
        return false;
    }
    if ch != '.' {
        return true;
    }
    // `...name(` is a spread call. `obj.name(` and `obj?.name(` stay member access.
    prefix.ends_with("...")
}

fn owner_name_shadowed_by_unrelated_import(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    test.imports_in_file
        .iter()
        .filter(|import| import.local == owner.name)
        .any(|import| {
            import.namespace
                || !import_source_matches_owner(
                    import,
                    &test.file,
                    owner,
                    alias_map,
                    workspace_root,
                )
                || import.imported.as_deref().is_some_and(|imported| {
                    imported != owner.name.as_str() && imported != "default"
                })
        })
}

fn owner_export_imported_from_unrelated_source(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    test.imports_in_file.iter().any(|import| {
        import.imported.as_deref() == Some(owner.name.as_str())
            && !import_source_matches_owner(import, &test.file, owner, alias_map, workspace_root)
    })
}

fn import_references_owner_call(
    import: &TypeScriptImport,
    body_text: &str,
    owner: &TypeScriptOwner,
) -> bool {
    if import.namespace {
        return contains_member_call_name(body_text, &import.local, &owner.name);
    }
    // A default import (`import local from './owner'`) binds the owner
    // module's default export (#4103 under-credit): credit a call through the
    // local when the owner IS that default export. An anonymous default
    // already matches via `owner.name == "default"`.
    import.imported.as_deref().is_some_and(|imported| {
        imported == owner.name || (imported == "default" && owner.exported_as_default)
    }) && contains_call_name(body_text, &import.local)
        // Shadow guard (#4102): the same test-body declaration guard the
        // alias arm applies — a locally re-declared `local` reaches the
        // shadow, not the owner.
        && !local_identifier_declared_in_test_body(body_text, &import.local)
}

pub(crate) fn import_source_matches_owner(
    import: &TypeScriptImport,
    test_file: &Path,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    normalized_relative_import_module(test_file, &import.source, alias_map, workspace_root)
        .is_some_and(|module| module == normalized_module_path(&owner.file))
}

/// Resolve an import specifier relative to `test_file` to a normalized module
/// path string (no extension, forward-slash separated).
///
/// For relative specifiers (`./` or `../`), performs the usual path-join.
///
/// For non-relative specifiers, consults `alias_map` when provided:
/// - If the alias map successfully resolves the specifier to a unique workspace
///   file, returns that file's normalized module path.
/// - Otherwise returns `None` (fail-closed).
pub(crate) fn normalized_relative_import_module(
    test_file: &Path,
    source: &str,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> Option<String> {
    if source.starts_with("./") || source.starts_with("../") {
        // Standard relative resolution.
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
        return Some(strip_typescript_module_extension(&parts.join("/")));
    }

    // Non-relative specifier — consult alias map if available.
    let alias_map = alias_map?;
    let root = workspace_root?;
    let _ = root; // root is captured by alias_map already
    let resolved_path = alias_map.resolve(source)?;
    // Normalize the resolved workspace-relative path to a module string.
    let normalized = normalized_path(&resolved_path);
    Some(strip_typescript_module_extension(&normalized))
}

fn normalized_module_path(path: &Path) -> String {
    strip_typescript_module_extension(&normalized_path(path))
}

fn strip_typescript_module_extension(path: &str) -> String {
    for suffix in [".tsx", ".mts", ".cts", ".ts", ".jsx", ".mjs", ".cjs", ".js"] {
        if let Some(stripped) = path.strip_suffix(suffix) {
            return stripped.to_string();
        }
    }
    path.to_string()
}

fn contains_member_call_name(body_text: &str, object_name: &str, method_name: &str) -> bool {
    if !is_safe_javascript_identifier(object_name) || !is_safe_javascript_identifier(method_name) {
        return false;
    }
    // Plain, optional-chain (`obj?.m(`) and non-null (`obj!.m(`) member calls
    // all reach the same member of the same object binding (#4103
    // under-credit: the punctuated forms were invisible to every member arm).
    [
        format!("{object_name}.{method_name}("),
        format!("{object_name}?.{method_name}("),
        format!("{object_name}!.{method_name}("),
    ]
    .iter()
    .any(|needle| {
        body_text.match_indices(needle.as_str()).any(|(idx, _)| {
            has_member_call_boundary(body_text, idx)
                && !line_prefix_looks_like_comment_or_string(body_text, idx)
                && !inside_block_comment(body_text, idx)
        })
    })
}

/// `true` when `body_text` contains an `object.property` MEMBER READ — the
/// property is not extended into a longer identifier and is not called.
/// Getter owners are invoked by such reads, so this is their call needle
/// (#4104-B). `cart.total(` (an actual call) is deliberately NOT a read
/// match; the call needle covers it.
pub(crate) fn contains_member_read_name(
    body_text: &str,
    object_name: &str,
    property_name: &str,
) -> bool {
    if !is_safe_javascript_identifier(object_name) || !is_safe_javascript_identifier(property_name)
    {
        return false;
    }
    let needle = format!("{object_name}.{property_name}");
    body_text.match_indices(&needle).any(|(idx, _)| {
        has_member_call_boundary(body_text, idx)
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
            && !inside_block_comment(body_text, idx)
            && body_text
                .get(idx + needle.len()..)
                .and_then(|rest| rest.chars().next())
                .is_none_or(|ch| !is_javascript_identifier_char(ch) && ch != '(')
    })
}

/// `true` when `body_text` constructs `class_name` with `new class_name(`.
/// Boundary-guarded on both sides: `newX(` and `renew Wallet(` do not match,
/// `new WalletFactory(` does not credit a `Wallet` constructor, and
/// `new Wallet.Provider(` is a different constructor. Fail-closed on generic
/// type arguments (`new Wallet<Foo>(`) — a missed relation is honest, a
/// wrong one is not.
pub(crate) fn contains_new_expression_call(body_text: &str, class_name: &str) -> bool {
    if !is_safe_javascript_identifier(class_name) {
        return false;
    }
    let needle = format!("new {class_name}");
    body_text.match_indices(&needle).any(|(idx, _)| {
        // The `new` keyword must not be the tail of a longer identifier.
        let new_starts_clean = idx == 0
            || body_text[..idx]
                .chars()
                .next_back()
                .is_none_or(|ch| !is_javascript_identifier_char(ch));
        // The next non-whitespace character must open the argument list.
        let tail_opens_call = body_text
            .get(idx + needle.len()..)
            .and_then(|rest| rest.trim_start().chars().next())
            .is_some_and(|ch| ch == '(');
        new_starts_clean
            && tail_opens_call
            && !line_prefix_looks_like_comment_or_string(body_text, idx)
            && !inside_block_comment(body_text, idx)
    })
}

fn expect_actual_references_identifier(body_text: &str, identifier: &str) -> bool {
    is_safe_javascript_identifier(identifier)
        && expect_actual_slices(body_text).iter().any(|actual| {
            actual.trim_start().starts_with(identifier)
                && actual
                    .trim_start()
                    .get(identifier.len()..)
                    .and_then(|rest| rest.chars().next())
                    .is_none_or(|ch| !is_javascript_identifier_char(ch))
        })
}

fn expect_actual_references_member(
    body_text: &str,
    object_name: &str,
    property_name: &str,
) -> bool {
    if !is_safe_javascript_identifier(object_name) || !is_safe_javascript_identifier(property_name)
    {
        return false;
    }
    // Same punctuated-access family as `contains_member_reference` (#4103):
    // `ns.owner`, `ns?.owner`, `ns!.owner` are the same member read.
    [
        format!("{object_name}.{property_name}"),
        format!("{object_name}?.{property_name}"),
        format!("{object_name}!.{property_name}"),
    ]
    .iter()
    .any(|reference| {
        expect_actual_slices(body_text).iter().any(|actual| {
            actual.trim_start().starts_with(reference.as_str())
                && actual
                    .trim_start()
                    .get(reference.len()..)
                    .and_then(|rest| rest.chars().next())
                    .is_none_or(|ch| !is_javascript_identifier_char(ch))
        })
    })
}

fn expect_actual_slices(body_text: &str) -> Vec<&str> {
    body_text
        .match_indices("expect(")
        .filter_map(|(idx, _)| {
            if line_prefix_looks_like_comment_or_string(body_text, idx)
                || inside_block_comment(body_text, idx)
            {
                return None;
            }
            body_text.get(idx + "expect(".len()..)
        })
        .collect()
}

/// A local declaration of the shadow-checked identifier found by the lexical
/// scope walk in `local_identifier_declared_in_test_body`.
struct BodyDeclaration {
    /// Byte offset of the declared name token, so the walk does not count the
    /// declaration's own binding as a use of the identifier.
    name_start: usize,
    /// Enclosing block scope (index into the walk's scope table); `None` is
    /// the test body's top level, which shadows every use in the body.
    scope: Option<usize>,
}

/// Lexical states for the shadow-check walk.
#[derive(Clone, Copy)]
enum BodyScanState {
    Code,
    LineComment,
    BlockComment,
    SingleQuoted,
    DoubleQuoted,
    Template,
}

/// Whether a local declaration of `identifier` shadows one of its uses in the
/// test body (#4102 shadow guard, scope-aware per the #4117 review): a
/// declaration at the body's top level shadows every use (call or bare
/// identifier reference), while a declaration inside a nested block shadows
/// only the uses inside that same block — a `const owner` buried in a helper
/// branch must not reject an imported-owner call made outside it. The walk is
/// a conservative lexer that skips comments and string/template contents
/// (template interpolations resume code tracking, and a multi-line comment
/// counts as a statement break for the line-start check); regex literals are
/// not modeled. Uses reuse the module's reference predicates: no member
/// access (`x.owner`), no object-literal key. Every declaration kind shadows
/// each use lexically inside its recorded scope regardless of textual order
/// (#4117 review TDZ fix): `function` declarations hoist within their block,
/// `var` hoists to the enclosing function, and `const`/`let` bind their whole
/// block — a same-block use before the declaration sits in the temporal dead
/// zone and can never reach an outer owner. Known conservatism: a nested-block
/// `var` is recorded at its own block rather than the enclosing function, so
/// a same-function use outside that block stays unshadowed (over-credit
/// direction, retained under the cross-block interval rules).
pub(crate) fn local_identifier_declared_in_test_body(body_text: &str, identifier: &str) -> bool {
    // Scope table: (open-brace offset, close-brace offset; `usize::MAX` while
    // the scope is still open). Entries are kept after closing so a nested
    // declaration can be tested for enclosing a specific use.
    let mut scopes: Vec<(usize, usize)> = Vec::new();
    let mut open_scopes: Vec<usize> = Vec::new();
    let mut uses: Vec<usize> = Vec::new();
    let mut declarations: Vec<BodyDeclaration> = Vec::new();
    let mut state = BodyScanState::Code;
    // Depths whose closing `}` returns the walk from a template
    // interpolation (which opened a pseudo-scope) to its template literal.
    let mut interpolation_returns: Vec<usize> = Vec::new();
    let mut at_line_start = true;
    let mut escaped = false;
    let mut pending_dollar = false;
    let mut block_comment_has_newline = false;
    for (idx, ch) in body_text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\n' {
            match state {
                BodyScanState::LineComment => state = BodyScanState::Code,
                BodyScanState::SingleQuoted | BodyScanState::DoubleQuoted => {
                    // Unterminated string: recover instead of mis-scoping the
                    // remainder of the body.
                    state = BodyScanState::Code;
                    at_line_start = true;
                }
                BodyScanState::BlockComment => block_comment_has_newline = true,
                BodyScanState::Template => pending_dollar = false,
                BodyScanState::Code => at_line_start = true,
            }
            continue;
        }
        match state {
            BodyScanState::Code => match ch {
                c if c.is_whitespace() => {}
                '/' if body_text[idx + 1..].starts_with('/') => state = BodyScanState::LineComment,
                '/' if body_text[idx + 1..].starts_with('*') => state = BodyScanState::BlockComment,
                '\'' => state = BodyScanState::SingleQuoted,
                '"' => state = BodyScanState::DoubleQuoted,
                '`' => state = BodyScanState::Template,
                '{' => {
                    scopes.push((idx, usize::MAX));
                    open_scopes.push(scopes.len() - 1);
                    at_line_start = false;
                }
                '}' => {
                    if interpolation_returns.last() == Some(&open_scopes.len()) {
                        // `}` closing a `${ ... }` interpolation.
                        open_scopes.pop();
                        interpolation_returns.pop();
                        state = BodyScanState::Template;
                    } else if let Some(frame) = open_scopes.pop() {
                        scopes[frame].1 = idx;
                    }
                    at_line_start = false;
                }
                _ => {
                    if at_line_start
                        && let Some(keyword) = ["const ", "let ", "var ", "function "]
                            .into_iter()
                            .find(|keyword| body_text[idx..].starts_with(*keyword))
                    {
                        let line_end = body_text[idx..]
                            .find('\n')
                            .map_or(body_text.len(), |relative| idx + relative);
                        let line = &body_text[idx..line_end];
                        if declaration_line_declares_identifier(line, identifier) {
                            // Skip the declaration's own name token when
                            // collecting uses (leading whitespace after the
                            // keyword included).
                            let name_start = body_text[idx + keyword.len()..]
                                .char_indices()
                                .find(|(_, ch)| !ch.is_whitespace())
                                .map_or(idx + keyword.len(), |(relative, _)| {
                                    idx + keyword.len() + relative
                                });
                            declarations.push(BodyDeclaration {
                                name_start,
                                scope: open_scopes.last().copied(),
                            });
                        }
                    }
                    // A use of the identifier: a call or bare reference with
                    // the module's reference boundaries — never a member
                    // access (`x.owner`), object-literal key, or the declared
                    // name token itself. Comment/string occurrences cannot
                    // reach this arm (the state machine skips them).
                    if body_text[idx..].starts_with(identifier)
                        && has_member_call_boundary(body_text, idx)
                        && body_text[idx + identifier.len()..]
                            .chars()
                            .next()
                            .is_none_or(|ch| !is_javascript_identifier_char(ch))
                        && !is_object_literal_key(body_text, idx, idx + identifier.len())
                        && declarations
                            .last()
                            .is_none_or(|declaration| declaration.name_start != idx)
                    {
                        uses.push(idx);
                    }
                    at_line_start = false;
                }
            },
            BodyScanState::LineComment => {}
            BodyScanState::BlockComment => {
                if ch == '*' && body_text[idx + 1..].starts_with('/') {
                    // A multi-line comment counts as a statement break for the
                    // line-start check (ASI), so a declaration following it is
                    // still recognized as leading its line.
                    state = BodyScanState::Code;
                    at_line_start = block_comment_has_newline;
                }
            }
            BodyScanState::SingleQuoted => {
                if ch == '\'' {
                    state = BodyScanState::Code;
                    at_line_start = false;
                }
            }
            BodyScanState::DoubleQuoted => {
                if ch == '"' {
                    state = BodyScanState::Code;
                    at_line_start = false;
                }
            }
            BodyScanState::Template => {
                if ch == '`' {
                    state = BodyScanState::Code;
                    at_line_start = false;
                    pending_dollar = false;
                } else if ch == '$' {
                    pending_dollar = true;
                } else if ch == '{' && pending_dollar {
                    interpolation_returns.push(open_scopes.len() + 1);
                    scopes.push((idx, usize::MAX));
                    open_scopes.push(scopes.len() - 1);
                    state = BodyScanState::Code;
                    pending_dollar = false;
                } else {
                    pending_dollar = false;
                }
            }
        }
    }
    uses.iter().any(|use_offset| {
        declarations
            .iter()
            .any(|declaration| match declaration.scope {
                // Top-level declarations shadow every use in the body.
                None => true,
                // Otherwise the declaration shadows a use exactly when the use is
                // lexically inside the declaration's own scope: same-block
                // position is irrelevant (`function` hoists; `var` hoists to the
                // enclosing function; `const`/`let` bind block-wide, so a
                // pre-declaration same-block use sits in the temporal dead zone
                // and can never reach an outer binding), while a use outside the
                // recorded block still reaches the owner.
                Some(frame) => scopes
                    .get(frame)
                    .is_some_and(|(start, end)| *start <= *use_offset && *use_offset < *end),
            })
    })
}

fn declaration_line_declares_identifier(line: &str, identifier: &str) -> bool {
    ["const ", "let ", "var ", "function "]
        .into_iter()
        .filter_map(|keyword| line.strip_prefix(keyword))
        .filter_map(|after| {
            after
                .split(|ch: char| {
                    ch == ':'
                        || ch == '='
                        || ch == '('
                        || ch == ','
                        || ch == ';'
                        || ch.is_whitespace()
                })
                .find(|part| !part.is_empty())
        })
        .any(|declared| declared == identifier)
}

pub(crate) fn has_member_call_boundary(body_text: &str, idx: usize) -> bool {
    body_text[..idx]
        .chars()
        .next_back()
        .is_none_or(|ch| !is_javascript_identifier_char(ch) && ch != '.')
}

pub(crate) fn line_prefix_looks_like_comment_or_string(body_text: &str, idx: usize) -> bool {
    let line_start = body_text[..idx].rfind('\n').map_or(0, |offset| offset + 1);
    let prefix = &body_text[line_start..idx];
    prefix.trim_start().starts_with("//") || has_unclosed_quote_or_template(prefix)
}

pub(crate) fn inside_block_comment(body_text: &str, idx: usize) -> bool {
    let prefix = &body_text[..idx];
    let comment_start = prefix.rfind("/*");
    let comment_end = prefix.rfind("*/");
    comment_start.is_some_and(|start| comment_end.is_none_or(|end| start > end))
}

fn has_unclosed_quote_or_template(prefix: &str) -> bool {
    let mut escaped = false;
    let mut in_single = false;
    let mut in_double = false;
    let mut in_template = false;
    for ch in prefix.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\'' && !in_double && !in_template {
            in_single = !in_single;
        } else if ch == '"' && !in_single && !in_template {
            in_double = !in_double;
        } else if ch == '`' && !in_single && !in_double {
            in_template = !in_template;
        }
    }
    in_single || in_double || in_template
}

pub(crate) fn is_javascript_identifier_char(ch: char) -> bool {
    ch == '_' || ch == '$' || ch.is_ascii_alphanumeric()
}

fn same_file_proximity_related(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    let Some(owner_stem) = owner.file.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let Some(test_stem) = test.file.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    let owner_key = normalize_typescript_test_stem(owner_stem);
    let test_key = normalize_typescript_test_stem(test_stem);
    !owner_key.is_empty() && owner_key == test_key
}

fn normalize_typescript_test_stem(stem: &str) -> String {
    let mut value = stem.to_string();
    for suffix in [".test", ".spec", "_test", "-test"] {
        if let Some(stripped) = value.strip_suffix(suffix) {
            value = stripped.to_string();
            break;
        }
    }
    for prefix in ["test.", "test_", "test-"] {
        if let Some(stripped) = value.strip_prefix(prefix) {
            value = stripped.to_string();
            break;
        }
    }
    normalize_similarity_key(&value)
}

fn describe_name_similar_to_owner(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    test.describe_names.iter().any(|name| {
        let describe_key = normalize_similarity_key(name);
        owner_similarity_keys(owner)
            .into_iter()
            .any(|key| similarity_key_contains(&describe_key, &key))
    })
}

fn test_name_similar_to_owner(test: &TypeScriptTest, owner: &TypeScriptOwner) -> bool {
    let test_key = normalize_similarity_key(&test.local_name);
    owner_similarity_keys(owner)
        .into_iter()
        .any(|key| similarity_key_contains(&test_key, &key))
}

fn owner_similarity_keys(owner: &TypeScriptOwner) -> Vec<String> {
    let mut keys = Vec::new();
    push_unique_similarity_key(&mut keys, normalize_similarity_key(&owner.name));
    keys
}

fn push_unique_similarity_key(keys: &mut Vec<String>, key: String) {
    if !key.is_empty() && !keys.iter().any(|existing| existing == &key) {
        keys.push(key);
    }
}

fn normalize_similarity_key(input: &str) -> String {
    let mut out = String::new();
    let mut last_was_separator = true;
    let mut previous_was_lower_or_digit = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase()
                && !out.is_empty()
                && !last_was_separator
                && previous_was_lower_or_digit
            {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
            last_was_separator = false;
            previous_was_lower_or_digit = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        } else if !out.is_empty() && !last_was_separator {
            out.push('_');
            last_was_separator = true;
            previous_was_lower_or_digit = false;
        } else {
            previous_was_lower_or_digit = false;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

fn similarity_key_contains(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    haystack == needle
        || haystack
            .strip_prefix(needle)
            .is_some_and(|suffix| suffix.starts_with('_'))
        || haystack
            .strip_suffix(needle)
            .is_some_and(|prefix| prefix.ends_with('_'))
        || haystack.contains(&format!("_{needle}_"))
}

#[cfg(test)]
mod spread_call_boundary_tests {
    use super::contains_call_name;

    #[test]
    fn spread_is_a_call_and_member_access_is_not() -> Result<(), String> {
        let cases = [
            ("[...steps(4)]", true),
            ("Array.from(steps(4))", true),
            ("steps(4)", true),
            ("obj.steps(4)", false),
            ("obj?.steps(4)", false),
            ("notsteps(4)", false),
            ("// steps(4)", false),
            ("const steps = 1; steps(4)", true),
        ];
        for (text, expected) in cases {
            let actual = contains_call_name(text, "steps");
            if actual != expected {
                return Err(format!("{text}: expected call={expected}, got {actual}"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod module_extension_tests {
    use super::*;

    /// Every routed TS/JS suffix must strip to the same module identity, so an
    /// import that names `../src/cart.mjs` still credits a `src/cart.mts`
    /// owner. Pinned literally (not read back from the suffix list) so
    /// deleting a suffix from the production list fails this test.
    #[test]
    fn strip_typescript_module_extension_covers_every_routed_suffix() {
        let cases = [
            ("src/cart.ts", "src/cart"),
            ("src/cart.tsx", "src/cart"),
            ("src/cart.js", "src/cart"),
            ("src/cart.jsx", "src/cart"),
            ("src/cart.mts", "src/cart"),
            ("src/cart.cts", "src/cart"),
            ("src/cart.mjs", "src/cart"),
            ("src/cart.cjs", "src/cart"),
        ];
        for (input, expected) in cases {
            assert_eq!(
                strip_typescript_module_extension(input),
                expected,
                "{input} must normalize to the same module identity as every other routed suffix"
            );
        }
    }

    /// Near-miss and non-routed suffixes must NOT be stripped: an unrouted
    /// suffix such as `.mtsx` is left alone rather than partially matching, and
    /// a non-TypeScript module keeps its own identity.
    #[test]
    fn strip_typescript_module_extension_leaves_near_miss_and_unrouted_suffixes() {
        for input in [
            "src/cart.mtsx",
            "src/cart.ctss",
            "src/cart.mjss",
            "src/cart.rs",
            "src/cart.py",
            "src/cart",
        ] {
            assert_eq!(
                strip_typescript_module_extension(input),
                input,
                "{input} is not a routed module suffix and must keep its identity"
            );
        }
    }

    /// Declaration-stem stability: adding `.mts`/`.cts` must not change how a
    /// `.d.*` declaration module is identified. `cart.d.mts` resolves exactly
    /// like the long-standing `cart.d.ts` case ΓÇö the routed suffix is removed
    /// and the `.d` stem is preserved rather than rewritten to `cart`.
    #[test]
    fn strip_typescript_module_extension_keeps_declaration_stem_behavior() {
        for (declaration, control) in [
            ("src/cart.d.mts", "src/cart.d.ts"),
            ("src/cart.d.cts", "src/cart.d.ts"),
            ("src/cart.d.mjs", "src/cart.d.js"),
            ("src/cart.d.cjs", "src/cart.d.js"),
        ] {
            assert_eq!(
                strip_typescript_module_extension(declaration),
                strip_typescript_module_extension(control),
                "{declaration} must identify like its {control} control"
            );
            assert_eq!(strip_typescript_module_extension(declaration), "src/cart.d");
        }
    }

    /// A relative import that names a NEW suffix resolves to the owner's
    /// module identity. This is the cross-extension seam the whole routed
    /// surface depends on: without `.mjs`/`.cjs`/`.mts`/`.cts` here, a modern
    /// import silently fails to match its owner and the related test is lost.
    #[test]
    fn normalized_relative_import_module_matches_new_suffix_specifier_to_owner()
    -> Result<(), String> {
        for (test_file, source, owner_file) in [
            ("tests/cart.test.cts", "../src/cart.cjs", "src/cart.cts"),
            ("tests/cart.test.mts", "../src/cart.mjs", "src/cart.mts"),
            ("tests/cart.test.ts", "../src/cart.mjs", "src/cart.mts"),
            ("tests/cart.test.cjs", "../src/cart.mts", "src/cart.cts"),
        ] {
            let resolved =
                normalized_relative_import_module(Path::new(test_file), source, None, None)
                    .ok_or_else(|| format!("{source} from {test_file} must resolve"))?;
            // Assert the module identity LITERALLY. Deriving the expectation via
            // `normalized_module_path` would run both sides of the comparison
            // through the same stripper, so any *consistent* regression in
            // `strip_typescript_module_extension` (double-suffix stripping,
            // directory normalization, `index`-stem handling) would keep this
            // test green. Every routed specifier here denotes `src/cart`.
            const EXPECTED: &str = "src/cart";
            if resolved != EXPECTED {
                return Err(format!(
                    "{source} from {test_file} resolved to {resolved}, not the {owner_file} owner module {EXPECTED}"
                ));
            }
        }
        Ok(())
    }

    /// Cross-extension resolution must not over-credit: an import naming a
    /// DIFFERENT module is not the owner, even when both use modern suffixes.
    ///
    /// The discriminating assertion here is `resolved == "src/basket"`. A bare
    /// "is not the cart owner" inequality would stay green even if the entire
    /// routed-suffix list were emptied, because then nothing would be stripped
    /// and the specifier would trivially differ. Pinning the literal means this
    /// control fails if `.mjs` stripping regresses. The over-credit decision
    /// itself is pinned end to end by
    /// `tests::analyze_diff_does_not_credit_related_test_from_a_different_modern_module`.
    #[test]
    fn normalized_relative_import_module_rejects_different_new_suffix_owner() -> Result<(), String>
    {
        let resolved = normalized_relative_import_module(
            Path::new("tests/cart.test.mts"),
            "../src/basket.mjs",
            None,
            None,
        )
        .ok_or("a relative modern-suffix specifier must still normalize")?;
        if resolved != "src/basket" {
            return Err(format!(
                "../src/basket.mjs must normalize to src/basket, not {resolved}"
            ));
        }
        if resolved == "src/cart" {
            return Err(format!(
                "a different modern module must not resolve to the cart owner module {resolved}"
            ));
        }
        Ok(())
    }
}
