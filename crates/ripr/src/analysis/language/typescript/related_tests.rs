//! Related test candidate discovery for the TypeScript preview adapter.

use super::tsconfig::TsAliasMap;
use super::*;
use std::collections::{HashMap, HashSet};

// ── Re-export index ───────────────────────────────────────────────────────────

/// Maximum number of re-export hops followed from the test's import source
/// toward the owner module (RIPR-SPEC-0095). A chain longer than this is left
/// unresolved (fail-closed): the test is not credited through it.
pub(crate) const MAX_REEXPORT_HOPS: usize = 4;

/// Bounded in-repo re-export index built during Phase 1 of the adapter.
///
/// Records, per normalized non-test module:
/// - explicit named re-exports `export { N [as M] } from './A'`
///   as `(module, M) -> (N, A_module)`, plus the module's own rename exports
///   `export { local as M }` as a self-hop `(module, M) -> (local, module)`;
/// - star re-exports `export * from './A'` as `(module, A_module)` edges;
/// - the names the module exports from its OWN declarations
///   (`export function N`, `export const N`, `export { N }`,
///   `export default ...` as `default`);
/// - the set of known workspace modules, so a directory specifier
///   (`import { N } from '../src'`) resolves to `src/index` only when no
///   `src.ts`-style file module exists (the TypeScript/Node lookup order).
///
/// A test that imports `N` from module B is credited when resolving `N`
/// through B's re-export chain, at most [`MAX_REEXPORT_HOPS`] hops, lands on
/// the changed owner's own export in the owner module. A star hop forwards a
/// name only when the target module actually exports it, a name exported by
/// more than one star source is ambiguous, and cycles or over-deep chains
/// stay unresolved — every uncertain case fails closed (no credit). The index
/// is empty when no re-exports are present, which makes all callers that pass
/// `ReExportIndex::empty()` behave identically to the pre-fix behaviour.
#[derive(Debug, Default, Clone)]
pub(crate) struct ReExportIndex {
    /// key: (intermediate_module_norm, exported_name)
    /// value: (original_name, source_module_norm)
    entries: HashMap<(String, String), (String, String)>,
    /// `export * from './source'` hops: (intermediate_module, star_source_module).
    /// A star re-export forwards every named export (never `default`) of the
    /// source module under its original name (#4103 under-credit: the star
    /// barrel was invisible and the emitted guidance claimed no test
    /// referenced the owner at all).
    star_edges: HashSet<(String, String)>,
    /// Names each module exports from its own declarations.
    local_exports: HashMap<String, HashSet<String>>,
    /// Normalized module paths of every workspace file seen by the index.
    modules: HashSet<String>,
}

/// Outcome of resolving one exported name through the re-export graph.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ExportResolution {
    /// The module (and its bounded star closure) does not export the name.
    NotFound,
    /// The name resolves to `(module, local_name)` — the module whose own
    /// declaration (or own rename export) binds it.
    Found(String, String),
    /// The chain is too deep, ambiguous, or otherwise not decidable.
    Unknown,
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

    /// Test-only constructor from raw entries, star edges, and the modules'
    /// own exported names `(module, name)`, so re-export resolution can be
    /// exercised without a real filesystem. Every module named anywhere in
    /// the parts is recorded as a known workspace module.
    #[cfg(test)]
    pub(crate) fn from_parts(
        entries: Vec<((String, String), (String, String))>,
        star_edges: Vec<(String, String)>,
        local_exports: Vec<(String, String)>,
    ) -> Self {
        let mut index = Self::default();
        for ((module, exported), (original, source)) in entries {
            index.modules.insert(module.clone());
            index.modules.insert(source.clone());
            index.entries.insert((module, exported), (original, source));
        }
        for (module, source) in star_edges {
            index.modules.insert(module.clone());
            index.modules.insert(source.clone());
            index.star_edges.insert((module, source));
        }
        for (module, name) in local_exports {
            index.modules.insert(module.clone());
            index.local_exports.entry(module).or_default().insert(name);
        }
        index
    }

    /// Build a re-export index from all non-test source files in the workspace.
    ///
    /// Indexes explicit named re-exports and star re-exports from relative
    /// (or tsconfig-aliased) in-repo specifiers, plus each module's own
    /// exported names. `export * as ns from`, type-only re-exports and
    /// non-relative package specifiers are ignored (fail-closed).
    ///
    /// `sources` is the Phase-1 workspace source cache: every file is read at
    /// most once per analysis run. Files absent from the cache (unreadable or
    /// over the read caps) contribute no export facts; their limitation
    /// disclosure is owned by the read pipeline.
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
        let mut local_exports: HashMap<String, HashSet<String>> = HashMap::new();
        let modules: HashSet<String> = workspace_files
            .iter()
            .map(|file| normalized_module_path(file))
            .collect();
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
                        return ModuleExportFacts::default();
                    }
                    module_export_facts(
                        relative,
                        &ret.program.body,
                        worker_alias.as_ref(),
                        worker_root.as_path(),
                    )
                },
            ) else {
                continue;
            };
            for (key, value) in file_facts.entries {
                entries.entry(key).or_insert_with(|| value);
            }
            star_edges.extend(file_facts.star_edges);
            if !file_facts.local_exports.is_empty() {
                local_exports
                    .entry(normalized_module_path(relative))
                    .or_default()
                    .extend(file_facts.local_exports);
            }
        }
        Self {
            entries,
            star_edges,
            local_exports,
            modules,
        }
    }

    /// Resolve a normalized module specifier path to the workspace module it
    /// names. A file module (`src.ts` → `src`) wins; otherwise a directory
    /// with an index file (`src/index.ts` → `src/index`) is used. When
    /// neither is known the path is returned unchanged (it then matches no
    /// indexed module and nothing is credited through it).
    fn canonical_module(&self, module: &str) -> String {
        if self.modules.contains(module) {
            return module.to_string();
        }
        let index_module = if module.is_empty() {
            "index".to_string()
        } else {
            format!("{module}/index")
        };
        if self.modules.contains(&index_module) {
            return index_module;
        }
        module.to_string()
    }

    /// Resolve `name` as exported by `module` through the bounded re-export
    /// graph. `stack` holds the `(module, name)` pairs being resolved on the
    /// current path (cycle guard).
    fn resolve_export(
        &self,
        module: &str,
        name: &str,
        hops: usize,
        stack: &mut Vec<(String, String)>,
        mocked: &dyn Fn(&str) -> bool,
    ) -> ExportResolution {
        let module = self.canonical_module(module);
        if mocked(&module) {
            // A mocked barrel or intermediate module replaces every binding
            // it forwards, so the chain no longer reaches the owner.
            return ExportResolution::Unknown;
        }
        let frame = (module.clone(), name.to_string());
        if stack.contains(&frame) {
            // A re-export cycle contributes no binding (ECMAScript
            // ResolveExport returns null for a revisited pair).
            return ExportResolution::NotFound;
        }
        stack.push(frame);
        let resolution = self.resolve_export_frame(&module, name, hops, stack, mocked);
        stack.pop();
        resolution
    }

    fn resolve_export_frame(
        &self,
        module: &str,
        name: &str,
        hops: usize,
        stack: &mut Vec<(String, String)>,
        mocked: &dyn Fn(&str) -> bool,
    ) -> ExportResolution {
        if let Some((original, source)) = self.entries.get(&(module.to_string(), name.to_string()))
        {
            if source == module {
                // Own rename export `export { local as name }` (#4104-B).
                return ExportResolution::Found(module.to_string(), original.clone());
            }
            if hops >= MAX_REEXPORT_HOPS {
                return ExportResolution::Unknown;
            }
            return match self.resolve_export(source, original, hops + 1, stack, mocked) {
                // An explicit `export { N } from './A'` asserts that A
                // exports N; when A records no deeper fact for it, A's own
                // binding is the named target.
                ExportResolution::NotFound => {
                    ExportResolution::Found(self.canonical_module(source), original.clone())
                }
                resolved => resolved,
            };
        }
        if self
            .local_exports
            .get(module)
            .is_some_and(|names| names.contains(name))
        {
            return ExportResolution::Found(module.to_string(), name.to_string());
        }
        // `export *` never forwards the default export.
        if name == "default" {
            return ExportResolution::NotFound;
        }
        let mut star_sources: Vec<&String> = self
            .star_edges
            .iter()
            .filter(|(from, _)| from == module)
            .map(|(_, to)| to)
            .collect();
        if star_sources.is_empty() {
            return ExportResolution::NotFound;
        }
        if hops >= MAX_REEXPORT_HOPS {
            return ExportResolution::Unknown;
        }
        star_sources.sort();
        let mut found: Option<(String, String)> = None;
        for star_source in star_sources {
            match self.resolve_export(star_source, name, hops + 1, stack, mocked) {
                ExportResolution::NotFound => {}
                ExportResolution::Unknown => return ExportResolution::Unknown,
                ExportResolution::Found(target_module, target_name) => {
                    let target = (target_module, target_name);
                    match &found {
                        None => found = Some(target),
                        Some(existing) if existing == &target => {}
                        // Two star sources export the same name with
                        // different bindings: ambiguous, not forwarded.
                        Some(_) => return ExportResolution::Unknown,
                    }
                }
            }
        }
        match found {
            Some((target_module, target_name)) => {
                ExportResolution::Found(target_module, target_name)
            }
            None => ExportResolution::NotFound,
        }
    }

    /// Return whether `test` importing `imported_name` from `import_source`
    /// reaches `owner` through the bounded re-export chain (or through a
    /// directory specifier that resolves to the owner's `index` module). A
    /// mock of any module on the chain breaks it.
    fn resolve_to_owner(
        &self,
        test: &TypeScriptTest,
        import_source: &str,
        imported_name: &str,
        owner: &TypeScriptOwner,
        alias_map: Option<&TsAliasMap>,
        workspace_root: Option<&Path>,
    ) -> bool {
        // Resolve the import source to a normalized module path.
        let Some(import_module) =
            normalized_relative_import_module(&test.file, import_source, alias_map, workspace_root)
        else {
            return false;
        };
        let owner_module = normalized_module_path(&owner.file);
        // The import names the owner module directly. A plain same-name
        // import is owned by the direct/imported-call relations (with their
        // own identity guards); only an owner-file RENAME export
        // (`export { ownerName as alias }`) needs the index here, because the
        // test imports `alias`, not the owner name (#4104-B).
        if import_module == owner_module {
            let key = (import_module, imported_name.to_string());
            let Some((original_name, source_module)) = self.entries.get(&key) else {
                return false;
            };
            return source_module == &owner_module && original_name == &owner.name;
        }
        if test
            .mocks_in_file
            .iter()
            .any(|source| source == UNRESOLVED_MOCK_SPECIFIER)
        {
            return false;
        }
        let mocked_modules: Vec<String> = test
            .mocks_in_file
            .iter()
            .filter_map(|source| {
                normalized_relative_import_module(&test.file, source, alias_map, workspace_root)
            })
            .map(|module| self.canonical_module(&module))
            .collect();
        let mocked = |module: &str| {
            mocked_modules.iter().any(|mocked| mocked == module)
                || test
                    .mocks_in_file
                    .iter()
                    .any(|source| root_relative_mock_names_module(source, module))
        };
        let mut stack = Vec::new();
        let ExportResolution::Found(target_module, target_name) =
            self.resolve_export(&import_module, imported_name, 0, &mut stack, &mocked)
        else {
            return false;
        };
        // The chain must land on the owner's file and the owner's name.
        // `export { default as X } from './owner'` lands on `default`; it
        // resolves to THIS owner exactly when the owner is the owner module's
        // default export (#4103 under-credit).
        target_module == owner_module
            && (target_name == owner.name
                || (target_name == "default" && owner.exported_as_default))
    }
}

/// Export facts of one parsed module, collected on the parse worker.
#[derive(Debug, Default)]
struct ModuleExportFacts {
    entries: Vec<((String, String), (String, String))>,
    star_edges: Vec<(String, String)>,
    local_exports: Vec<String>,
}

fn module_export_facts(
    relative: &Path,
    body: &oxc_allocator::Vec<'_, Statement<'_>>,
    alias_map: Option<&TsAliasMap>,
    workspace_root: &Path,
) -> ModuleExportFacts {
    // intermediate module path (normalized, no extension)
    let module = normalized_module_path(relative);
    let mut facts = ModuleExportFacts::default();
    for stmt in body {
        match stmt {
            Statement::ExportAllDeclaration(export_all) => {
                // `export * from './owner'` — record the star hop.
                // `export * as ns from './owner'` namespaces the module value,
                // so the named-forwarding contract a star hop carries does not
                // apply; it stays unindexed (fail-closed), as does a
                // type-only `export type * from`.
                if export_all.exported.is_some()
                    || export_all.export_kind == ImportOrExportKind::Type
                {
                    continue;
                }
                let star_source = export_all.source.value.to_string();
                if let Some(resolved) = normalized_relative_import_module(
                    relative,
                    &star_source,
                    alias_map,
                    Some(workspace_root),
                ) {
                    facts.star_edges.push((module.clone(), resolved));
                }
            }
            Statement::ExportDefaultDeclaration(_) => {
                facts.local_exports.push("default".to_string());
            }
            Statement::ExportNamedDeclaration(export) => {
                if export.export_kind == ImportOrExportKind::Type {
                    continue;
                }
                if let Some(declaration) = &export.declaration {
                    push_declaration_export_names(declaration, &mut facts.local_exports);
                    continue;
                }
                let Some(re_source) = &export.source else {
                    // Own export list `export { a, b as c }` (no `from`).
                    // A same-name entry exports the module's own binding; a
                    // RENAME (#4104-B) re-binds the file's OWN local binding
                    // under `exported_name` — recorded as a self-hop so
                    // `import { c } from './owner-file'` resolves back to the
                    // original owner name.
                    for specifier in &export.specifiers {
                        if specifier.export_kind == ImportOrExportKind::Type {
                            continue;
                        }
                        let Some(local_name) = module_export_name_text(&specifier.local) else {
                            continue;
                        };
                        let exported_name = module_export_name_text(&specifier.exported)
                            .unwrap_or_else(|| local_name.clone());
                        if exported_name == local_name {
                            facts.local_exports.push(local_name);
                            continue;
                        }
                        let key = (module.clone(), exported_name);
                        facts.entries.push((key, (local_name, module.clone())));
                    }
                    continue;
                };
                let source_str = re_source.value.to_string();
                // Resolve the source module relative to the intermediate
                // file's dir. Pass alias_map so tsconfig-aliased paths can be
                // followed.
                let Some(resolved) = normalized_relative_import_module(
                    relative,
                    &source_str,
                    alias_map,
                    Some(workspace_root),
                ) else {
                    continue;
                };
                for specifier in &export.specifiers {
                    if specifier.export_kind == ImportOrExportKind::Type {
                        continue;
                    }
                    let Some(original_name) = module_export_name_text(&specifier.local) else {
                        continue;
                    };
                    let exported_name = module_export_name_text(&specifier.exported)
                        .unwrap_or_else(|| original_name.clone());
                    // key: what the test would import from the intermediate file
                    let key = (module.clone(), exported_name);
                    // value: what the source file exports under its original name
                    facts.entries.push((key, (original_name, resolved.clone())));
                }
            }
            _ => {}
        }
    }
    facts
}

/// Value names bound by an exported declaration. Type-only declarations
/// (`interface`, `type`) bind no runtime value and are skipped.
fn push_declaration_export_names(declaration: &Declaration<'_>, names: &mut Vec<String>) {
    match declaration {
        Declaration::VariableDeclaration(variables) => {
            for declarator in &variables.declarations {
                for identifier in declarator.id.get_binding_identifiers() {
                    names.push(identifier.name.as_str().to_string());
                }
            }
        }
        Declaration::TSTypeAliasDeclaration(_) | Declaration::TSInterfaceDeclaration(_) => {}
        other => {
            if let Some(identifier) = other.id() {
                names.push(identifier.name.as_str().to_string());
            }
        }
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
/// `reexport_index` enables bounded re-export tracing: tests that import
/// the owner through a barrel file are credited when the chain resolves
/// in-source within `MAX_REEXPORT_HOPS` hops.  Pass `&ReExportIndex::empty()`
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
    // Same-module entry reach is admitted only when no test calls the owner
    // itself, so a direct relation's oracle alone decides exposure.
    if candidates.is_empty() && !owner.module_entries.is_empty() {
        candidates = all_tests
            .iter()
            .filter(|test| {
                workspace_root
                    .map(|root| same_package_root(&owner.file, &test.file, root))
                    .unwrap_or(true)
            })
            .filter(|test| {
                module_entry_relation(test, owner, reexport_index, alias_map, workspace_root)
            })
            .map(|test| TypeScriptRelatedCandidate {
                test,
                relation: TypeScriptRelationKind::ModuleEntryCall,
            })
            .collect();
    }
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
    // #4103 shape 4: a `vi.spyOn`/`jest.spyOn` on the owner name combined
    // with a value fabrication (`.mockReturnValue(...)` /
    // `.mockImplementation(...)`) replaces the owner implementation — the
    // observed value is the fabrication, not the changed sink, so no trusted
    // owner-call relation may be credited. A bare spyOn (call-through) is
    // unaffected and still relates.
    if test_spies_owner_with_fabrication(test, &owner.name) {
        return None;
    }
    if contains_call_name(&test.body_text, &owner.name)
        && !owner_name_shadowed_by_unrelated_import(test, owner, alias_map, workspace_root)
        // Shadow guard (#4102): a test body that declares its own
        // `function <owner>(...)` / `const <owner> = ...` (or a destructured
        // property binding of the name) calls the local declaration, not the
        // imported owner — do not credit DirectOwnerCall.
        && !local_identifier_declared_in_test_body(&test.body_text, &owner.name)
        && !enclosing_scope_shadows(test, &owner.name)
        && !owner_name_destructured_from_unrelated_source(test, owner, alias_map, workspace_root)
        // #4103 shape 1: a bare `ownerName(...)` only counts when a
        // declaration anchors the name to the owner (same file, or an import
        // of the owner from the owner's module).
        && direct_owner_call_has_declaration_anchor(test, owner, alias_map, workspace_root)
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
            && !enclosing_scope_shadows(test, &import.local)
    }) {
        return Some(TypeScriptRelationKind::ImportAliasOwnerCall);
    }
    // Named (or namespace) import of the owner from the owner's module with a
    // call through the import's local binding. The same identity guards as the
    // direct-call arm apply: a body-local declaration of the import's local
    // name (#4102 shape 4 / #4103 shape 4 relation side) shadows the call, and
    // a body-local destructure of the owner name from an unrelated source
    // (#4103 shape 1) binds the bare call to that source — neither may be
    // credited as an imported owner call.
    if test.imports_in_file.iter().any(|import| {
        if !import_source_matches_owner(import, &test.file, owner, alias_map, workspace_root) {
            return false;
        }
        if local_identifier_declared_in_test_body(&test.body_text, &import.local)
            || enclosing_scope_shadows(test, &import.local)
        {
            return false;
        }
        if !import.namespace
            && owner_name_destructured_from_unrelated_source(test, owner, alias_map, workspace_root)
        {
            return false;
        }
        import_references_owner_call(import, &test.body_text, owner)
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
    // Bounded re-export tracing (RIPR-SPEC-0095):
    // If the test imports a name from a barrel (named `export { N } from` or
    // `export * from` hops, including a directory specifier that resolves to
    // its `index` module) whose chain lands on the owner's own export, credit
    // the test via re_export_chain_followed. At most `MAX_REEXPORT_HOPS` hops
    // are followed; deeper, cyclic or ambiguous chains stay uncredited
    // (fail-closed). The index also carries the owner file's OWN rename exports
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
            return false; // default imports are out of scope for re-export tracing
        }
        // The local alias in the test is what gets called; check the call site.
        let local = &import.local;
        if !contains_call_name(&test.body_text, local) {
            return false;
        }
        // Shadow guard (#4102): a test-body declaration of the local name
        // reaches the shadow, not the re-exported owner.
        if local_identifier_declared_in_test_body(&test.body_text, local) {
            return false;
        }
        // An enclosing `describe`/`beforeEach` binding of the local name
        // shadows the import for every test in that scope.
        if enclosing_scope_shadows(test, local) {
            return false;
        }
        reexport_index.resolve_to_owner(
            test,
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

/// Whether the test calls an exported name of the owner's module that reaches
/// the owner (`TypeScriptOwner::module_entries`). Each entry is checked as if
/// it were the changed owner, so the test must satisfy every identity gate a
/// direct or imported owner call does (declaration anchor, shadowing, mocks,
/// spy fabrication, import source). The stand-in owner carries no entries of
/// its own, so reach through entries is never chained across modules.
fn module_entry_relation(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    reexport_index: &ReExportIndex,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    owner.module_entries.iter().any(|entry| {
        let entry_owner = module_entry_owner(owner, entry);
        owner_call_relation(
            test,
            &entry_owner,
            reexport_index,
            alias_map,
            workspace_root,
        )
        .is_some_and(TypeScriptRelationKind::uses_oracle)
    })
}

/// The owner as a test of `entry` sees it: same module, the entry's exported
/// name, a plain callable, and no entries of its own.
pub(crate) fn module_entry_owner(
    owner: &TypeScriptOwner,
    entry: &TypeScriptModuleEntry,
) -> TypeScriptOwner {
    TypeScriptOwner {
        name: entry.name.clone(),
        owner_kind: OwnerKind::Function,
        class_name: None,
        exported_as_default: entry.exported_as_default,
        class_default_export: false,
        method_kind: TypeScriptMethodKind::Ordinary,
        module_entries: Vec::new(),
        params: Vec::new(),
        arity: None,
        source_text: None,
        ..owner.clone()
    }
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
    let mut receiver_names =
        receiver_names_for_constructor_calls(&test.body_text, &constructor_names);
    // A receiver built in the enclosing scope (`const cart = new Cart()` in a
    // `describe`, `cart = new Cart()` in a `beforeEach`) is the object the
    // body calls, unless the body declares or reassigns that name itself.
    for binding in &test.scope_bindings {
        if binding
            .constructed_by
            .as_ref()
            .is_some_and(|constructor| constructor_names.contains(constructor))
            && !local_identifier_declared_in_test_body(&test.body_text, &binding.name)
            && !super::tests_extract::identifier_written_in(&test.body_text, &binding.name)
        {
            push_unique_string(&mut receiver_names, binding.name.clone());
        }
    }
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

/// `true` when a binding outside the test body but inside the file-level
/// scope (a `describe` body or parameter, a loop header, a hook write, a
/// test callback parameter) rebinds `name` for this test, so a call through
/// `name` reaches that binding rather than an import or owner of the name.
fn enclosing_scope_shadows(test: &TypeScriptTest, name: &str) -> bool {
    test.scope_bindings
        .iter()
        .any(|binding| binding.name == name && !binding.file_level)
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
        && !enclosing_scope_shadows(test, class_name)
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
            && !enclosing_scope_shadows(test, &import.local)
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
        if !import_source_matches_owner(import, &test.file, owner, alias_map, workspace_root) {
            continue;
        }
        if import.namespace {
            // `import * as shop from './cart'` + `new shop.Cart()`. A
            // default-exported class is `shop.default`, never `shop.Cart`.
            if !owner.class_default_export {
                push_unique_string(&mut names, format!("{}.{class_name}", import.local));
            }
        } else if import.imported.as_deref() == Some(class_name)
            || (owner.class_default_export && import.imported.as_deref() == Some("default"))
        {
            // A named import, or a default import of the owner's
            // default-exported class under any local name.
            push_unique_string(&mut names, import.local.clone());
        }
    }
    // A constructor name the test scope or body rebinds (`const Cart =
    // class {}`, `const shop = {...}`) constructs something else.
    names.retain(|name| {
        let local = name.split('.').next().unwrap_or(name);
        !test
            .scope_bindings
            .iter()
            .any(|binding| binding.name == local)
            && !local_identifier_declared_in_test_body(&test.body_text, local)
    });
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
    let owner_module = normalized_module_path(&owner.file);
    // A mock of a directory (`../src/cart`) replaces its `index` module.
    let owner_modules = [
        Some(owner_module.as_str()),
        owner_module.strip_suffix("/index"),
    ];
    test.mocks_in_file.iter().any(|source| {
        source == UNRESOLVED_MOCK_SPECIFIER
            || owner_modules.iter().flatten().any(|module| {
                root_relative_mock_names_module(source, module)
                    || normalized_relative_import_module(
                        &test.file,
                        source,
                        alias_map,
                        workspace_root,
                    )
                    .is_some_and(|mocked| mocked == *module)
            })
    })
}

/// `true` when a root-relative mock specifier (`/src/cart`) may name
/// `module`. The runner resolves it against its own root, which the adapter
/// does not model, so any whole-segment suffix of the module path matches:
/// an unknown root errs toward withholding the relation (#4294). A query
/// (`?raw`), empty and `.` segments are dropped first, so `//src/./cart`
/// reads as `src/cart`; a `..` segment makes the path unknowable and matches.
fn root_relative_mock_names_module(source: &str, module: &str) -> bool {
    let source = source.replace('\\', "/");
    let Some(rooted) = source.strip_prefix('/') else {
        return false;
    };
    let rooted = rooted.split(['?', '#']).next().unwrap_or_default();
    let segments: Vec<&str> = rooted
        .split('/')
        .filter(|segment| !segment.is_empty() && *segment != ".")
        .collect();
    if segments.contains(&"..") {
        return true;
    }
    let rooted = strip_typescript_module_extension(&segments.join("/"));
    !rooted.is_empty()
        && (module == rooted
            || module
                .strip_suffix(rooted.as_str())
                .is_some_and(|prefix| prefix.ends_with('/')))
}

/// Fabrication method shapes that replace a spy's observed value (#4103
/// shape 4).
const SPY_FABRICATION_SHAPES: [&str; 4] = [
    ".mockReturnValue(",
    ".mockImplementation(",
    ".mockResolvedValue(",
    ".mockRejectedValue(",
];

/// #4103 shape 4: `true` when the test body spies on the owner name
/// (`vi.spyOn(module, 'ownerName')` / `jest.spyOn(...)`) AND fabricates a
/// value FOR THAT SPY: the fabrication method is chained directly after the
/// owner `spyOn(...)` call closes (`vi.spyOn(mod, 'owner').mockReturnValue(v)`)
/// or invoked on the variable the spy is bound to
/// (`const spy = spyOn(mod, 'owner'); spy.mockReturnValue(v)`).
///
/// The fabricated value is what an assertion on the spied name observes; the
/// changed sink never executes, so any owner-call credit through this test
/// is mock-fabricated evidence. A bare spyOn without a fabrication on its
/// own value still calls through to the owner and is NOT blocked, and a mock
/// on an unrelated object (`logger.mockReturnValue(...)`) must never refuse
/// the owner relation — the fabrication has to be tied to the owner spy.
pub(crate) fn test_spies_owner_with_fabrication(test: &TypeScriptTest, owner_name: &str) -> bool {
    let body = &test.body_text;
    if !body.contains("spyOn(") {
        return false;
    }
    ["vi.spyOn(", "jest.spyOn(", "spyOn("].iter().any(|needle| {
        body.match_indices(needle).any(|(idx, _)| {
            let after_open = &body[idx + needle.len()..];
            spy_call_targets_name(after_open, owner_name)
                && owner_spy_call_fabricated(body, idx, after_open)
        })
    })
}

/// Whether the owner-targeting `spyOn(...)` call whose needle starts at
/// `call_start` (with `after_open` the body right after the call's open
/// paren) fabricates its value: a fabrication shape chained directly after
/// the call closes, or invoked on the variable the spy result is bound to.
fn owner_spy_call_fabricated(body: &str, call_start: usize, after_open: &str) -> bool {
    let Some(close_rel) = spy_call_close_offset(after_open) else {
        return false;
    };
    let tail = after_open[close_rel + 1..].trim_start();
    let tail = tail.strip_prefix('?').unwrap_or(tail).trim_start();
    if SPY_FABRICATION_SHAPES
        .iter()
        .any(|shape| tail.starts_with(shape))
    {
        return true;
    }
    spy_bound_variable_name(body, call_start)
        .is_some_and(|variable| spy_variable_fabricates(body, &variable))
}

/// Byte offset of the `)` closing the call whose argument list starts at the
/// beginning of `after_open` (the call's own open paren sits before it).
/// Strings are skipped; bounded so an unterminated call cannot run away.
fn spy_call_close_offset(after_open: &str) -> Option<usize> {
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (offset, ch) in after_open.char_indices().take(2000) {
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == open {
                quote = None;
            }
            continue;
        }
        match ch {
            '"' | '\'' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' => {
                if depth == 0 {
                    return Some(offset);
                }
                depth = depth.saturating_sub(1);
            }
            ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

/// The variable name the `spyOn(...)` call starting at `call_start` is bound
/// to, when the call is the right-hand side of a simple `name = spyOn(...)`
/// (`const spy = spyOn(...)`, a bare `spy = spyOn(...)` assignment, with an
/// optional `await` and the needle's `vi.` / `jest.` qualifier). A member
/// tail (`obj.spy = ...`), a compound operator (`==`, `=>`), or any other
/// prefix returns `None` (conservative: only clear bindings count).
fn spy_bound_variable_name(body: &str, call_start: usize) -> Option<String> {
    let mut prefix = body[..call_start].trim_end();
    // The qualified needle forms leave the callee qualifier (`vi.` / `jest.`)
    // at the end of the prefix.
    if let Some(stripped) = prefix.strip_suffix('.') {
        let qualifier_len = stripped
            .chars()
            .rev()
            .take_while(|ch| is_javascript_identifier_char(*ch))
            .count();
        prefix = &stripped[..stripped.len() - qualifier_len];
    }
    if let Some(stripped) = strip_trailing_await_keyword(prefix) {
        prefix = stripped;
    }
    let assigned = prefix.strip_suffix('=')?.trim_end();
    // A compound operator ending in `=` (`==`, `===`, `!=`, `<=`, `>=`,
    // `=>`, `+=`, ...) never binds the call result.
    if assigned
        .chars()
        .next_back()
        .is_some_and(|ch| "=!<>+-*/%&|^:".contains(ch))
    {
        return None;
    }
    let variable: String = assigned
        .chars()
        .rev()
        .take_while(|ch| is_javascript_identifier_char(*ch))
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if variable.is_empty() {
        return None;
    }
    let head = &assigned[..assigned.len() - variable.len()];
    // A member tail (`obj.spy = ...`) or a longer identifier is not a simple
    // binding of the spy result.
    if head
        .chars()
        .next_back()
        .is_some_and(|ch| is_javascript_identifier_char(ch) || ch == '.')
    {
        return None;
    }
    Some(variable)
}

/// Strip a trailing standalone `await` keyword from an assignment prefix.
fn strip_trailing_await_keyword(prefix: &str) -> Option<&str> {
    let stripped = prefix.trim_end().strip_suffix("await")?;
    if stripped
        .chars()
        .next_back()
        .is_some_and(is_javascript_identifier_char)
    {
        return None; // part of a longer identifier, not the keyword
    }
    Some(stripped.trim_end())
}

/// Whether a fabrication shape is invoked on `variable` with identifier
/// boundaries (`spy.mockReturnValue(...)`, not `myspy.mockReturnValue(...)`).
fn spy_variable_fabricates(body: &str, variable: &str) -> bool {
    SPY_FABRICATION_SHAPES.iter().any(|shape| {
        let needle = format!("{variable}{shape}");
        body.match_indices(&needle)
            .any(|(idx, _)| has_member_call_boundary(body, idx))
    })
}

/// Whether the argument list after `spyOn(` names `owner_name` as the spied
/// method. Only the spyOn call's own argument list is scanned (up to its
/// matching close paren), so a `.mockReturnValue("...")` chained after the
/// call can never be mistaken for the spied name. The method name is the LAST
/// string literal of the argument list — `spyOn(module, 'name')` and
/// `spyOn(require('path'), 'name')` both put it there. Bounded so an
/// unterminated call cannot run away.
fn spy_call_targets_name(after_open: &str, owner_name: &str) -> bool {
    let bounded: String = after_open.chars().take(500).collect();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut body = String::new();
    let mut strings: Vec<String> = Vec::new();
    let mut closed = false;
    for ch in bounded.chars() {
        if closed {
            break;
        }
        if let Some(open) = quote {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == open {
                quote = None;
                strings.push(std::mem::take(&mut body));
            } else {
                body.push(ch);
            }
            continue;
        }
        match ch {
            '"' | '\'' | '`' => quote = Some(ch),
            '(' | '[' | '{' => depth += 1,
            ')' => {
                if depth == 0 {
                    closed = true;
                } else {
                    depth = depth.saturating_sub(1);
                }
            }
            ']' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    strings
        .iter()
        .rev()
        .find(|literal| !literal.is_empty())
        .is_some_and(|name| name == owner_name)
}

/// Initializer text of every body-local destructuring that binds the owner
/// name (`const { ownerName } = <init>`), one entry per matching declaration
/// line; comment lines are skipped.
fn owner_name_destructure_inits(test: &TypeScriptTest, owner_name: &str) -> Vec<String> {
    let body = &test.body_text;
    if !body.contains('{') {
        return Vec::new();
    }
    body.lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") {
                return None;
            }
            let after_keyword = ["const ", "let ", "var "]
                .into_iter()
                .find_map(|keyword| trimmed.strip_prefix(keyword))?;
            let open = after_keyword.find('{')?;
            let close_rel = after_keyword[open..].find('}')? + open;
            let binds_owner = after_keyword[open + 1..close_rel].split(',').any(|part| {
                part.split(':')
                    .next()
                    .map(str::trim)
                    .is_some_and(|name| name == owner_name)
            });
            if !binds_owner {
                return None;
            }
            Some(
                after_keyword[close_rel + 1..]
                    .split_once('=')
                    .map(|(_, rhs)| rhs.trim().trim_end_matches(';').trim())
                    .unwrap_or("")
                    .to_string(),
            )
        })
        .collect()
}

/// #4103 shape 1: whether a body-local destructuring
/// (`const { applyDiscount } = <init>`) binds the owner name to a source
/// other than the owner's own module. The bare `applyDiscount(...)` call then
/// reaches the destructured value — a helper or factory, not the owner — so
/// it must not credit `DirectOwnerCall`.
///
/// A destructuring FROM the owner's module (`require("../src/owner")`, or a
/// namespace import of it) is not a shadow: it anchors the call through
/// `owner_name_destructured_from_owner_source` instead.
fn owner_name_destructured_from_unrelated_source(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    owner_name_destructure_inits(test, &owner.name)
        .iter()
        .any(|init| {
            // `require("<path>")`: a shadow only when the path is NOT the
            // owner's own module.
            if let Some(source) = require_source_from_text(init) {
                return normalized_relative_import_module(
                    &test.file,
                    &source,
                    alias_map,
                    workspace_root,
                )
                .is_none_or(|module| module != normalized_module_path(&owner.file));
            }
            // `<namespace>` where the namespace import binds the owner's
            // module: not a shadow. Anything else (factory call, dynamic
            // import, an unrelated binding) is.
            let init_ident: String = init
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '$')
                .collect();
            !test.imports_in_file.iter().any(|import| {
                import.namespace
                    && import.local == init_ident
                    && import_source_matches_owner(
                        import,
                        &test.file,
                        owner,
                        alias_map,
                        workspace_root,
                    )
            })
        })
}

/// #4103 shape 1 (anchor complement): a body-local destructuring that binds
/// the owner name from the owner's own module — CommonJS
/// `const { owner } = require("../src/owner")`, or a destructure of a
/// recorded namespace import of the owner module — anchors the bare
/// `owner(...)` call to the owner exactly like a recorded import would. The
/// import layer extracts only top-level statements, so this body-local shape
/// must anchor itself or a real CommonJS test falls back to the heuristic
/// relation.
fn owner_name_destructured_from_owner_source(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    owner_name_destructure_inits(test, &owner.name)
        .iter()
        .any(|init| {
            if let Some(source) = require_source_from_text(init) {
                return normalized_relative_import_module(
                    &test.file,
                    &source,
                    alias_map,
                    workspace_root,
                )
                .is_some_and(|module| module == normalized_module_path(&owner.file));
            }
            let init_ident: String = init
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_' || *ch == '$')
                .collect();
            test.imports_in_file.iter().any(|import| {
                import.namespace
                    && import.local == init_ident
                    && import_source_matches_owner(
                        import,
                        &test.file,
                        owner,
                        alias_map,
                        workspace_root,
                    )
            })
        })
}

/// Extract the string-literal path from a `require("...")` call in `init`
/// text, when it is the simple literal form.
fn require_source_from_text(init: &str) -> Option<String> {
    let start = init.find("require(")? + "require(".len();
    let rest = init[start..].trim_start();
    let quote = rest.chars().next()?;
    if !matches!(quote, '"' | '\'') {
        return None;
    }
    let body = rest.strip_prefix(quote)?;
    let end = body.find(quote)?;
    Some(body[..end].to_string())
}

/// #4103 shape 1: `true` when a declaration anchors a bare `ownerName(...)`
/// call to the owner under analysis — the test lives in the owner's own
/// file, it imports the owner (named import, or a default/require binding
/// under the owner's name) from the owner's module, or a body-local
/// destructuring binds the owner name from the owner's module (a CommonJS
/// test keeps its `require` destructure inside the test body, which the
/// import layer does not record). A bare unimported call has no such anchor:
/// the same-named function may be defined anywhere, so the relation must not
/// credit it (fail-closed to the heuristic fallback).
fn direct_owner_call_has_declaration_anchor(
    test: &TypeScriptTest,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    if normalized_module_path(&test.file) == normalized_module_path(&owner.file) {
        return true;
    }
    test.imports_in_file.iter().any(|import| {
        import.local == owner.name
            && import_source_matches_owner(import, &test.file, owner, alias_map, workspace_root)
            && match import.imported.as_deref() {
                Some(name) => name == owner.name || name == "default",
                // A bare namespace binding without a default/name record is
                // not an anchor for a bare call.
                None => false,
            }
    }) || owner_name_destructured_from_owner_source(test, owner, alias_map, workspace_root)
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
/// `reexport_index` enables bounded re-export tracing.
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
            let strongest =
                candidate_observes_owner_call(&candidate, owner, alias_map, workspace_root)
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

/// Whether this candidate's test observes an owner-name call whose assertion
/// classification may be read independently of relation credit.
///
/// Oracle facts and relation credit are orthogonal (RIPR-SPEC-0027):
/// the #4102/#4103 relation gates decide whether a test's call may CREDIT
/// exposure, not whether the test's assertion shape may be CLASSIFIED. A
/// broad `toThrow` is broad-error/weak evidence and a smoke call is smoke
/// evidence regardless of whether the relation to the owner is anchored.
///
/// Trusted relations observe the owner by construction. A relation denied by
/// the gates still contains an owner-name call unless a gate found POSITIVE
/// evidence that the name does not reach the owner: a body-local declaration,
/// an unrelated import/destructure shadow, an owner-module mock, or a spy
/// fabrication all make the assertion observe a different function or a
/// fabricated value, so those assertions stay unclassified for the owner. A
/// missing declaration anchor alone is unproven rather than disproven — the
/// test really calls a function of the owner's name — so its assertion shape
/// stays readable while the finding discloses the uncertain relation and
/// withholds exposure credit.
pub(crate) fn candidate_observes_owner_call(
    candidate: &TypeScriptRelatedCandidate<'_>,
    owner: &TypeScriptOwner,
    alias_map: Option<&TsAliasMap>,
    workspace_root: Option<&Path>,
) -> bool {
    if candidate.relation.uses_oracle() {
        return true;
    }
    let test = candidate.test;
    contains_call_name(&test.body_text, &owner.name)
        && !local_identifier_declared_in_test_body(&test.body_text, &owner.name)
        && !owner_name_shadowed_by_unrelated_import(test, owner, alias_map, workspace_root)
        && !owner_name_destructured_from_unrelated_source(test, owner, alias_map, workspace_root)
        && !test_mocks_owner_module(test, owner, alias_map, workspace_root)
        && !test_spies_owner_with_fabrication(test, &owner.name)
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
        // The test calls a same-module export that calls the owner: the
        // TypeScript form of a production helper that delegates to the owner.
        TypeScriptRelationKind::ModuleEntryCall => RelationReason::HelperOwnerCall,
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
        | TypeScriptRelationKind::ReExportChainFollowed
        | TypeScriptRelationKind::ModuleEntryCall => RelationConfidence::Medium,
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
/// For relative specifiers (`./`, `../`, or exactly `.` / `..`), performs the
/// usual path-join. When `workspace_root` is known and the joined path names
/// an in-root directory with no sibling file module, the directory resolves
/// through [`resolve_directory_module`] (package.json `main`, else `index`),
/// mirroring the Node/TypeScript lookup order (#4546).
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
    // `.` and `..` name the current / parent directory module
    // (`require('..')`, #4546) exactly like `./` and `../` prefixes do.
    if is_relative_specifier(source) {
        // Standard relative resolution.
        let mut parts = normalized_path(test_file.parent().unwrap_or_else(|| Path::new("")))
            .split('/')
            .filter(|part| !part.is_empty() && *part != ".")
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let normalized_source = source.replace('\\', "/");
        let mut escaped_root = false;
        for part in normalized_source.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    escaped_root |= parts.pop().is_none();
                }
                _ => parts.push(part.to_string()),
            }
        }
        let joined = parts.join("/");
        let module = strip_typescript_module_extension(&joined);
        // Directory resolution only for an extensionless specifier whose
        // join stayed inside the workspace root; otherwise the historical
        // lexical module is kept.
        if let Some(root) = workspace_root
            && !escaped_root
            && module == joined
            && let Some(resolved) = resolve_directory_module(root, &module)
        {
            return Some(resolved);
        }
        return Some(module);
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

/// Extensions probed for a file module, in the TypeScript/Node lookup order.
const DIRECTORY_MODULE_EXTENSIONS: [&str; 8] =
    ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];

/// Resolve a workspace-relative module path that names a DIRECTORY to the
/// module the directory loads (#4546): the directory's `package.json` `main`
/// when present, else its `index` file.
///
/// Returns `None` (the caller keeps the lexical module) when the path is not
/// a real in-root directory (symlinks are not followed), when a sibling file
/// module (`<module>.<ext>`) exists and wins the lookup, or when nothing in
/// the directory resolves. A `package.json` that is unreadable (over the
/// config cap, not a regular file), is not valid JSON, or names a `main`
/// that is absolute, escapes the workspace root, or does not resolve to a
/// supported source file also yields `None`: the specifier stays unresolved
/// rather than being credited to a guessed module (fail-closed).
///
/// Memoized per analysis run by [`DirectoryModuleCacheScope`] (#4638
/// review): the owners x tests x imports loop asks the same question for
/// the same `(root, module)` many times, and each uncached answer costs
/// several filesystem probes.
fn resolve_directory_module(root: &Path, module: &str) -> Option<String> {
    let key = (root.to_path_buf(), module.to_string());
    let cached = DIRECTORY_MODULE_CACHE.with(|cache| {
        cache
            .try_borrow()
            .ok()
            .and_then(|cache| cache.as_ref().and_then(|map| map.get(&key).cloned()))
    });
    if let Some(resolved) = cached {
        return resolved;
    }
    let resolved = resolve_directory_module_uncached(root, module);
    DIRECTORY_MODULE_CACHE.with(|cache| {
        if let Ok(mut cache) = cache.try_borrow_mut()
            && let Some(map) = cache.as_mut()
        {
            map.insert(key, resolved.clone());
        }
    });
    resolved
}

/// Directory-module answers keyed by `(workspace root, module)`, alive only
/// while a [`DirectoryModuleCacheScope`] is open on this thread. Outside a
/// scope nothing is cached, so no answer outlives the run that computed it
/// (an LSP session sees filesystem changes on its next run).
type DirectoryModuleCache = HashMap<(PathBuf, String), Option<String>>;

thread_local! {
    static DIRECTORY_MODULE_CACHE: std::cell::RefCell<Option<DirectoryModuleCache>> =
        const { std::cell::RefCell::new(None) };
}

/// One analysis run's directory-module memo: opening it installs a fresh,
/// empty cache on this thread; dropping it restores whatever was there
/// before (normally nothing). The cache is an explicit per-run scope rather
/// than a parameter because `workspace_root` reaches the resolver through
/// ~30 signatures in the related-test and classifier walks.
pub(crate) struct DirectoryModuleCacheScope {
    previous: Option<DirectoryModuleCache>,
}

impl DirectoryModuleCacheScope {
    pub(crate) fn open() -> Self {
        let previous = DIRECTORY_MODULE_CACHE.with(|cache| {
            cache
                .try_borrow_mut()
                .ok()
                .and_then(|mut cache| cache.replace(HashMap::new()))
        });
        Self { previous }
    }
}

impl Drop for DirectoryModuleCacheScope {
    fn drop(&mut self) {
        let previous = self.previous.take();
        DIRECTORY_MODULE_CACHE.with(|cache| {
            if let Ok(mut cache) = cache.try_borrow_mut() {
                *cache = previous;
            }
        });
    }
}

fn resolve_directory_module_uncached(root: &Path, module: &str) -> Option<String> {
    let directory = if module.is_empty() {
        root.to_path_buf()
    } else {
        root.join(module)
    };
    if !is_real_directory(&directory) || file_module_exists(root, module) {
        return None;
    }
    let manifest = directory.join("package.json");
    match std::fs::symlink_metadata(&manifest) {
        // No package.json at all: plain directory index lookup.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return index_module(root, module);
        }
        Err(_) => return None,
        Ok(_) => {}
    }
    let text = read_config_capped(&manifest).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    match value.get("main") {
        Some(serde_json::Value::String(main)) => package_main_module(root, module, main),
        // No `main`: the directory falls back to its index file.
        None => index_module(root, module),
        // A non-string `main` is not a path Node would honour.
        Some(_) => None,
    }
}

/// The module a directory `package.json` `main` names, resolved inside the
/// workspace root: a file module (with or without a supported extension), or
/// the `index` of a directory it names.
fn package_main_module(root: &Path, module: &str, main: &str) -> Option<String> {
    let main = main.replace('\\', "/");
    if main.is_empty() || main.starts_with('/') || main.contains(':') {
        return None;
    }
    let mut parts = module
        .split('/')
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    for part in main.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                // Escaping the workspace root fails closed.
                parts.pop()?;
            }
            _ => parts.push(part.to_string()),
        }
    }
    let target = parts.join("/");
    if target.is_empty() {
        return None;
    }
    let stripped = strip_typescript_module_extension(&target);
    if stripped != target {
        // `main` carries a supported extension: it must name that file.
        return is_real_file(&root.join(&target)).then_some(stripped);
    }
    if file_module_exists(root, &target) {
        return Some(target);
    }
    if is_real_directory(&root.join(&target)) {
        return index_module(root, &target);
    }
    None
}

/// `<module>/index` when the directory holds an index file module.
fn index_module(root: &Path, module: &str) -> Option<String> {
    let index = if module.is_empty() {
        "index".to_string()
    } else {
        format!("{module}/index")
    };
    file_module_exists(root, &index).then_some(index)
}

/// Whether `<module>.<ext>` exists as a regular file for a supported
/// extension.
fn file_module_exists(root: &Path, module: &str) -> bool {
    !module.is_empty()
        && DIRECTORY_MODULE_EXTENSIONS
            .iter()
            .any(|extension| is_real_file(&root.join(format!("{module}.{extension}"))))
}

fn is_real_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_file())
}

fn is_real_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_dir())
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
/// scope walk in `scan_body_lexical_facts`.
pub(crate) struct BodyDeclaration {
    /// Byte offset of the declared name token, so the walk does not count the
    /// declaration's own binding as a use of the identifier.
    pub(crate) name_start: usize,
    /// Enclosing block scope (index into the walk's scope table); `None` is
    /// the test body's top level, which shadows every use in the body.
    pub(crate) scope: Option<usize>,
}

/// Scope intervals and identifier declarations/uses collected by one lexical
/// walk over a test body (`scan_body_lexical_facts`). Extracted from the
/// #4117 shadow walk so other resolvers reuse the SAME scope table instead
/// of re-deriving brace scoping.
pub(crate) struct BodyLexicalScan {
    /// `(open-brace offset, close-brace offset)` per `{`-scope; `usize::MAX`
    /// while a scope is still open. Entries are kept after closing.
    pub(crate) scopes: Vec<(usize, usize)>,
    pub(crate) declarations: Vec<BodyDeclaration>,
    pub(crate) uses: Vec<usize>,
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
/// branch must not reject an imported-owner call made outside it. The scan
/// is a conservative lexer that skips comments and string/template contents
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
    let scan = scan_body_lexical_facts(body_text, identifier);
    scan.uses.iter().any(|use_offset| {
        scan.declarations.iter().any(|declaration| {
            declaration_scope_binds(
                declaration
                    .scope
                    .and_then(|frame| scan.scopes.get(frame).copied()),
                *use_offset,
            )
        })
    })
}

/// The visibility decision for one declaration's scope against one use
/// offset, per the #4117 walk semantics: a top-level declaration (`None`)
/// binds every use in the body; a declaration inside a nested block binds a
/// use exactly when the use is lexically inside the declaration's own scope
/// interval — same-block position is irrelevant (`function` hoists; `var`
/// hoists to the enclosing function; `const`/`let` bind block-wide, so a
/// pre-declaration same-block use sits in the temporal dead zone and can
/// never reach an outer binding), while a use outside the recorded block
/// still reaches the outer binding. Shared by the shadow walk and the
/// observed-local resolver so the scope rules cannot drift between them
/// (#4213 review thread PRRT_kwDOSiSx0c6mUkkO).
pub(crate) fn declaration_scope_binds(
    declaration_scope: Option<(usize, usize)>,
    use_offset: usize,
) -> bool {
    match declaration_scope {
        None => true,
        Some((start, end)) => start <= use_offset && use_offset < end,
    }
}

/// The innermost brace scope recorded by the lexical walk that contains
/// `offset`, if any. Scopes are properly nested or disjoint, so the
/// innermost enclosing scope is the one with the greatest start.
pub(crate) fn innermost_scope_containing(
    scopes: &[(usize, usize)],
    offset: usize,
) -> Option<(usize, usize)> {
    scopes
        .iter()
        .copied()
        .filter(|(start, end)| *start <= offset && offset < *end)
        .max_by_key(|(start, _)| *start)
}

/// One conservative-lexer walk over a test body collecting the scope table,
/// every local declaration of `identifier`, and every use of it. See
/// `local_identifier_declared_in_test_body` for the full lexical contract.
pub(crate) fn scan_body_lexical_facts(body_text: &str, identifier: &str) -> BodyLexicalScan {
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
    BodyLexicalScan {
        scopes,
        declarations,
        uses,
    }
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
