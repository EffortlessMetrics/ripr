//! Contextual source-role composition across include and module edges (#3533).
//!
//! Per-file parsing classifies each function from its own file's syntax only.
//! Rust compilation crosses those boundaries: an out-of-line
//! `#[cfg(test)] mod tests;` makes every function in the child file
//! test-only, and a file-level `include!` pastes a fragment into the
//! including file's context. Before this pass, roles never crossed those
//! edges — ripr's own out-of-line test-module helpers classified as
//! `Production` and re-entered the production seam inventory.
//!
//! This pass runs **after** the style normalizer (which recomputes roles from
//! same-file text and would stomp any earlier composition) and composes roles
//! per occurrence under one closed rule:
//!
//! - **Evidence roles union; production requires both sides production.** A
//!   function already carrying an evidence role keeps it. A `Production`
//!   function inside a context that structurally requires a test build is
//!   granted `CfgTestModule`.
//! - **Composition only mints `CfgTestModule`** (evidence-only). It never
//!   creates or removes executable `TestFact`s — that stays behind the
//!   #3499/#3532 executable-test authority.
//! - **Unknown fails closed.** Ambiguous or cyclic module ownership,
//!   conflicting module/include contexts, dynamic `#[path]` targets, and
//!   unresolved targets grant nothing and name the earliest unresolved edge
//!   on the file's `SourceRoleProvenance`.
//! - **Dual contextual ownership** (one physical file both declared as a
//!   module and included as a fragment) composes only when both contexts
//!   agree; disagreement fails closed. The two-occurrence identity the issue
//!   prefers is not built here; the composed single-identity result stays
//!   conservative in the meantime.

use super::FunctionSourceRole;
use super::model::{
    ModuleDeclarationFact, ModulePathTarget, ResolvedIncludeParent, RustIndex, SourceRoleProvenance,
};
use super::{SourceRoleProvenanceEdge, SourceRoleProvenanceEdgeKind};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

/// Chain-walk bound, mirroring the include pass's depth limit. Module and
/// include chains past this bound are exotic input that fails closed instead
/// of risking unbounded recursion.
const MAX_CONTEXT_CHAIN_DEPTH: usize = 32;

/// Limitation reason: two distinct parent declarations resolve to the same
/// child file (or a default-resolution declaration matches both candidate
/// layouts), so no single contextual role can be composed.
pub(super) const REASON_MODULE_AMBIGUOUS_PARENT: &str = "rust_module_ambiguous_parent";
/// Limitation reason: the module/include context chain cycled or exceeded the
/// depth bound, so the context could not be resolved.
pub(super) const REASON_MODULE_CYCLE_OR_DEPTH_LIMIT: &str = "rust_module_cycle_or_depth_limit";
/// Limitation reason: the file is both a module child and an include fragment
/// and the two contexts disagree on test requirement.
pub(super) const REASON_MODULE_CONTEXT_CONFLICT: &str = "rust_module_context_conflict";

/// One resolved module-declaration edge: parent file declares an out-of-line
/// module whose child file exists in the index.
#[derive(Clone)]
struct ModuleEdge {
    parent: PathBuf,
    declaration: String,
    line: usize,
    requires_test: bool,
}

/// The effective test requirement of a file's composition context.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Context {
    /// Every recorded context edge resolved and none requires a test build.
    Production,
    /// At least one recorded context edge structurally requires a test build.
    RequiresTest,
    /// A context edge could not be resolved; the named reason is the earliest
    /// unresolved edge in the chain.
    Unresolved(&'static str),
}

impl Context {
    /// `Some(true)` when the context structurally requires a test build,
    /// `Some(false)` when it provably does not, `None` when unresolved.
    fn requires_test(&self) -> Option<bool> {
        match self {
            Context::RequiresTest => Some(true),
            Context::Production => Some(false),
            Context::Unresolved(_) => None,
        }
    }

    fn unresolved_reason(&self) -> Option<String> {
        match self {
            Context::Unresolved(reason) => Some((*reason).to_string()),
            _ => None,
        }
    }
}

/// Composes contextual source roles across the include and module edges of a
/// fully parsed, normalized index. Deterministic: all intermediate maps are
/// ordered, so grants and provenance are byte-identical across runs.
pub(super) fn compose_index_source_roles(index: &mut RustIndex, workspace_root: &Path) {
    let module_edges = resolved_module_edges(index, workspace_root);
    // Phase 1 (immutable): resolve every file's context and provenance.
    // The resolver borrows `include_parents`, so all mutations wait for it
    // to go out of scope.
    let resolutions: Vec<(PathBuf, Context, SourceRoleProvenance)> = {
        let mut resolver = ContextResolver {
            module_edges,
            include_parents: &index.include_parents,
            memo: BTreeMap::new(),
        };
        index
            .files
            .keys()
            .cloned()
            .map(|file| {
                let (context, provenance) = resolver.context_with_provenance(&file);
                (file, context, provenance)
            })
            .collect()
    };

    // Phase 2 (mutable): record provenance and grant evidence roles.
    let mut granted: BTreeSet<PathBuf> = BTreeSet::new();
    for (file, context, provenance) in resolutions {
        if context == Context::RequiresTest {
            granted.insert(file.clone());
        }
        let has_provenance =
            !provenance.edges.is_empty() || provenance.earliest_unresolved_reason.is_some();
        if has_provenance && let Some(facts) = index.files.get_mut(&file) {
            facts.role_provenance = provenance;
        }
    }
    if granted.is_empty() {
        return;
    }
    apply_cfg_test_module_grants(index, &granted);
}

/// Grants the evidence-only `CfgTestModule` role to `Production` functions of
/// the given files, in both the per-file facts and the flat function list.
/// Existing evidence roles (executable tests, promoted expansions, same-file
/// cfg-test helpers) are never demoted: evidence roles union.
fn apply_cfg_test_module_grants(index: &mut RustIndex, granted: &BTreeSet<PathBuf>) {
    for file in granted {
        let Some(facts) = index.files.get_mut(file) else {
            continue;
        };
        for function in &mut facts.functions {
            if function.source_role == FunctionSourceRole::Production {
                function.source_role = FunctionSourceRole::CfgTestModule;
            }
        }
    }
    for function in &mut index.functions {
        if granted.contains(&function.file)
            && function.source_role == FunctionSourceRole::Production
        {
            function.source_role = FunctionSourceRole::CfgTestModule;
        }
    }
}

/// Resolution outcome for one declaration's target path.
enum DeclarationTargets {
    /// Exactly one indexed child file.
    Exact(PathBuf),
    /// More than one indexed file matches the declaration (two claiming
    /// parents, or both default layouts present — the ambiguous E0761 shape).
    Ambiguous(Vec<PathBuf>),
    /// No indexed child (missing target, dynamic `#[path]`, or repository
    /// escape): no edge, fail closed.
    Unresolvable,
}

/// Resolves every indexed out-of-line module declaration to its child file,
/// keeping exactly one edge per child. Ambiguous resolution yields no edge:
/// the child keeps its standalone roles and records the ambiguity on its
/// provenance.
fn resolved_module_edges(
    index: &RustIndex,
    workspace_root: &Path,
) -> BTreeMap<PathBuf, Result<ModuleEdge, &'static str>> {
    let mut candidates: BTreeMap<PathBuf, BTreeSet<(PathBuf, usize, String, bool)>> =
        BTreeMap::new();
    let mut ambiguous: BTreeSet<PathBuf> = BTreeSet::new();
    let containing_directory_files = index
        .files
        .iter()
        .filter(|(_, facts)| !facts.used_lexical_fallback)
        .flat_map(|(file, facts)| {
            let directory = file.parent().unwrap_or_else(|| Path::new(""));
            facts
                .module_declarations
                .iter()
                .filter_map(move |declaration| {
                    let ModulePathTarget::Literal(literal) = &declaration.path_target else {
                        return None;
                    };
                    resolve_relative(directory, literal)
                })
        })
        .filter(|file| index.files.contains_key(file))
        .chain(index.include_parents.keys().cloned())
        .chain(index.include_targets.iter().cloned())
        .collect();
    let resolver = DeclarationResolver {
        index,
        directory_anchors: module_directory_anchors(
            index,
            workspace_root,
            &containing_directory_files,
        ),
    };

    for (file, facts) in &index.files {
        if facts.used_lexical_fallback || facts.module_declarations.is_empty() {
            continue;
        }
        // Both literal paths and default declarations inside include
        // fragments resolve from the physical fragment's directory. The
        // including unit remains an identity/role parent, not a file-search
        // anchor; rebasing it here can select a different physical child.
        for declaration in &facts.module_declarations {
            match resolver.declaration_targets(file, declaration) {
                DeclarationTargets::Unresolvable => {
                    // Dynamic, conditional, or out-of-repository `#[path]`
                    // targets grant nothing; there is no child identity to
                    // attach provenance to.
                }
                DeclarationTargets::Ambiguous(children) => {
                    ambiguous.extend(children);
                }
                DeclarationTargets::Exact(child) => {
                    candidates.entry(child).or_default().insert((
                        file.clone(),
                        declaration.line,
                        format!("mod {};", declaration.name),
                        declaration.requires_test,
                    ));
                }
            }
        }
    }

    let mut edges = BTreeMap::new();
    // Ambiguity is fail-closed per child: another declaration's multiple
    // default layouts mark the child ambiguous even when this declaration
    // has a single exact owner (two declarations can both reach the same
    // child file). The candidates loop runs first and skips ambiguous
    // children; the moved set then seeds their errors.
    for (child, owners) in &candidates {
        if owners.len() > 1 || ambiguous.contains(child) {
            edges.insert(child.clone(), Err(REASON_MODULE_AMBIGUOUS_PARENT));
        }
    }
    for child in ambiguous {
        edges
            .entry(child)
            .or_insert_with(|| Err(REASON_MODULE_AMBIGUOUS_PARENT));
    }
    for (child, owners) in candidates {
        if owners.len() > 1 || edges.contains_key(&child) {
            continue;
        }
        if let Some((parent, line, declaration, requires_test)) = owners.into_iter().next() {
            edges.insert(
                child,
                Ok(ModuleEdge {
                    parent,
                    declaration,
                    line,
                    requires_test,
                }),
            );
        }
    }
    edges
}

/// Retain each physical file's possible search directories. A file reached
/// through an ordinary module and a literal path/include can have two anchors.
/// Propagating both through default declarations prevents a descendant from
/// silently disappearing when the single-identity index cannot represent the
/// two occurrences separately. Each file has at most a containing-directory
/// and an ordinary stem-directory anchor, so the work queue remains bounded.
fn module_directory_anchors(
    index: &RustIndex,
    workspace_root: &Path,
    containing_directory_files: &BTreeSet<PathBuf>,
) -> BTreeMap<PathBuf, BTreeSet<PathBuf>> {
    let mut crate_roots = CrateRoots::default();
    let mut pending = index
        .files
        .iter()
        .filter(|(_, facts)| !facts.used_lexical_fallback && !facts.module_declarations.is_empty())
        .map(|(file, _)| {
            let file_name = file.file_name().and_then(|name| name.to_str());
            let directory = if containing_directory_files.contains(file)
                || matches!(file_name, Some("mod.rs" | "lib.rs" | "main.rs"))
                || crate_roots.is_crate_root(workspace_root, file)
            {
                directory_of(file)
            } else {
                ordinary_module_directory(file)
            };
            (file.clone(), directory)
        })
        .collect::<Vec<_>>();
    let mut anchors: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
    while let Some((file, directory)) = pending.pop() {
        let Some(facts) = index.files.get(&file) else {
            continue;
        };
        if facts.used_lexical_fallback || facts.module_declarations.is_empty() {
            continue;
        }
        if !anchors
            .entry(file.clone())
            .or_default()
            .insert(directory.clone())
        {
            continue;
        }
        for declaration in &facts.module_declarations {
            match &declaration.path_target {
                ModulePathTarget::Unknown => {}
                ModulePathTarget::Literal(literal) => {
                    if let Some(child) = resolve_relative(&directory_of(&file), literal)
                        && index.files.contains_key(&child)
                    {
                        pending.push((child.clone(), directory_of(&child)));
                    }
                }
                ModulePathTarget::Default => {
                    for child in default_candidates(&directory, &declaration.name) {
                        if index.files.contains_key(&child) {
                            pending.push((child.clone(), ordinary_module_directory(&child)));
                        }
                    }
                }
            }
        }
    }
    anchors
}

fn ordinary_module_directory(file: &Path) -> PathBuf {
    let directory = directory_of(file);
    if file.file_name().is_some_and(|name| name == "mod.rs") {
        return directory;
    }
    match file.file_stem() {
        Some(stem) => directory.join(stem),
        None => directory,
    }
}

fn default_candidates(directory: &Path, name: &str) -> [PathBuf; 2] {
    let name = name.strip_prefix("r#").unwrap_or(name);
    [
        directory.join(format!("{name}.rs")),
        directory.join(name).join("mod.rs"),
    ]
}

/// Resolves declarations against all directory anchors of their source file.
struct DeclarationResolver<'index> {
    index: &'index RustIndex,
    directory_anchors: BTreeMap<PathBuf, BTreeSet<PathBuf>>,
}

impl DeclarationResolver<'_> {
    /// Resolves one declaration against the index. An exact string-literal
    /// `#[path]` resolves relative to the directory of the file physically
    /// containing the declaration (the Rust reference rule for non-inline
    /// `#[path]` targets, which differs from the default stem-directory
    /// rule). Default resolution checks both `<module-dir>/<name>.rs` and
    /// `<module-dir>/<name>/mod.rs`, retaining ambiguity when both exist.
    fn declaration_targets(
        &self,
        physical_file: &Path,
        declaration: &ModuleDeclarationFact,
    ) -> DeclarationTargets {
        match &declaration.path_target {
            ModulePathTarget::Unknown => DeclarationTargets::Unresolvable,
            ModulePathTarget::Literal(literal) => {
                match resolve_relative(&directory_of(physical_file), literal) {
                    Some(child) if self.index.files.contains_key(&child) => {
                        DeclarationTargets::Exact(child)
                    }
                    _ => DeclarationTargets::Unresolvable,
                }
            }
            ModulePathTarget::Default => {
                let Some(directories) = self.directory_anchors.get(physical_file) else {
                    return DeclarationTargets::Unresolvable;
                };
                let indexed = directories
                    .iter()
                    .flat_map(|directory| default_candidates(directory, &declaration.name))
                    .filter(|candidate| self.index.files.contains_key(candidate))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                if directories.len() > 1 && !indexed.is_empty() {
                    // Even if only one candidate exists in this index, the
                    // other occurrence has an unresolved physical context.
                    return DeclarationTargets::Ambiguous(indexed);
                }
                match indexed.as_slice() {
                    [] => DeclarationTargets::Unresolvable,
                    [single] => DeclarationTargets::Exact(single.clone()),
                    // Both default layouts indexed at once: ambiguous.
                    [..] => DeclarationTargets::Ambiguous(indexed.clone()),
                }
            }
        }
    }
}

/// Crate-root identity for default module resolution (#3533 review).
///
/// Rust resolves `mod name;` in a crate root relative to the root file's
/// containing directory regardless of the root's filename, while an ordinary
/// module file resolves children under the directory named after its own
/// stem. Treating every non-`lib.rs`/`main.rs` file as an ordinary module
/// mis-anchors a custom-named crate root (`[lib] path = "source/root.rs"`)
/// and every integration-test, bench, and example target file.
///
/// Crate roots are recognized statically, in two fail-closed ways:
/// - layout autodiscovery, relative to the nearest ancestor directory that
///   contains a `Cargo.toml`: `src/lib.rs`, `src/main.rs`, one file directly
///   under `src/bin/`, and one file directly under `tests/`, `benches/`, or
///   `examples/`;
/// - manifest declaration: any `path = ...` on `[lib]`, `[[bin]]`,
///   `[[test]]`, `[[bench]]`, or `[[example]]` of that manifest.
///
/// A file with no ancestor manifest keeps the stem-directory rule (the
/// status quo for fixture trees and manifest-less scans). Every lookup is
/// memoized for the composition pass, so the filesystem cost is bounded by
/// the distinct declaring files and their bounded ancestor walks.
#[derive(Default)]
struct CrateRoots {
    verdicts: BTreeMap<PathBuf, bool>,
    manifest_dirs: BTreeMap<PathBuf, Option<PathBuf>>,
    declared_roots: BTreeMap<PathBuf, Vec<PathBuf>>,
}

impl CrateRoots {
    fn is_crate_root(&mut self, workspace_root: &Path, file: &Path) -> bool {
        if let Some(verdict) = self.verdicts.get(file) {
            return *verdict;
        }
        let verdict = self.compute_is_crate_root(workspace_root, file);
        self.verdicts.insert(file.to_path_buf(), verdict);
        verdict
    }

    fn compute_is_crate_root(&mut self, workspace_root: &Path, file: &Path) -> bool {
        let Some(package_dir) = self.package_manifest_dir(workspace_root, file) else {
            return false;
        };
        let Ok(relative) = file.strip_prefix(&package_dir) else {
            return false;
        };
        if is_layout_autodiscovered_root(relative) {
            return true;
        }
        let declared = self.manifest_declared_roots(workspace_root, &package_dir);
        declared
            .iter()
            .any(|target| package_dir.join(target) == file)
    }

    /// The nearest ancestor directory (including the workspace root itself)
    /// that contains a `Cargo.toml`, memoized per directory. Walks are
    /// depth-bounded: exotic nesting past the bound keeps the stem rule.
    ///
    /// Every directory visited by a walk is memoized with the walk's resolved
    /// answer, not just with "this directory itself has no manifest" (#3533
    /// review): a cached `None` for an intermediate directory would stop a
    /// later sibling's walk before it reached an ancestor manifest, so two
    /// files in the same directory could disagree on crate-root identity
    /// depending on index iteration order.
    fn package_manifest_dir(&mut self, workspace_root: &Path, file: &Path) -> Option<PathBuf> {
        let mut cursor = file.parent().map(Path::to_path_buf)?;
        let mut visited: Vec<PathBuf> = Vec::new();
        let resolved: Option<PathBuf> = 'walk: {
            for _ in 0..=MAX_CONTEXT_CHAIN_DEPTH {
                if let Some(found) = self.manifest_dirs.get(&cursor) {
                    break 'walk found.clone();
                }
                if workspace_root.join(&cursor).join("Cargo.toml").is_file() {
                    self.manifest_dirs
                        .insert(cursor.clone(), Some(cursor.clone()));
                    break 'walk Some(cursor);
                }
                visited.push(cursor.clone());
                if !cursor.pop() {
                    break 'walk None;
                }
            }
            // Depth bound exhausted: exotic nesting keeps the fail-closed
            // no-manifest answer.
            None
        };
        // Every visited directory shares this walk's nearest-ancestor answer:
        // they are all strict descendants of the directory it resolved at.
        for directory in visited {
            self.manifest_dirs.insert(directory, resolved.clone());
        }
        resolved
    }

    fn manifest_declared_roots(
        &mut self,
        workspace_root: &Path,
        package_dir: &Path,
    ) -> Vec<PathBuf> {
        if let Some(declared) = self.declared_roots.get(package_dir) {
            return declared.clone();
        }
        let declared = std::fs::read_to_string(workspace_root.join(package_dir).join("Cargo.toml"))
            .map(|text| {
                crate::analysis::workspace::declared_crate_root_paths_from_manifest(
                    &text,
                    package_dir,
                )
            })
            .unwrap_or_default();
        self.declared_roots
            .insert(package_dir.to_path_buf(), declared.clone());
        declared
    }
}

/// True when `relative` (a file path relative to its package directory)
/// matches Cargo's autodiscovered target layout: `src/lib.rs`, `src/main.rs`,
/// one file directly under `src/bin/`, or one file directly under `tests/`,
/// `benches/`, or `examples/`. Deeper files are module children, not roots.
fn is_layout_autodiscovered_root(relative: &Path) -> bool {
    let name_of = |component: &std::path::Component| match component {
        Component::Normal(name) => Some(name.to_string_lossy().to_string()),
        _ => None,
    };
    let components: Vec<_> = relative.components().collect();
    match components.as_slice() {
        [first, file_name] => {
            let (Some(dir), Some(file_name)) = (name_of(first), name_of(file_name)) else {
                return false;
            };
            match dir.as_str() {
                "src" => matches!(file_name.as_str(), "lib.rs" | "main.rs"),
                "tests" | "benches" | "examples" => file_name.ends_with(".rs"),
                _ => false,
            }
        }
        [first, second, file_name] => {
            let (Some(first), Some(second), Some(file_name)) =
                (name_of(first), name_of(second), name_of(file_name))
            else {
                return false;
            };
            first == "src" && second == "bin" && file_name.ends_with(".rs")
        }
        _ => false,
    }
}

fn directory_of(file: &Path) -> PathBuf {
    file.parent().map(Path::to_path_buf).unwrap_or_default()
}

/// Resolves a `#[path]` literal against a base directory, rejecting absolute
/// targets and repository escapes the same way the include pass does.
fn resolve_relative(base: &Path, literal: &str) -> Option<PathBuf> {
    if Path::new(literal).is_absolute() {
        return None;
    }
    let mut normalized = base.to_path_buf();
    for component in Path::new(literal).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!normalized.as_os_str().is_empty()).then_some(normalized)
}

/// Resolves each file's effective context with memoization, cycle guarding,
/// and a depth bound, producing the provenance chain alongside the verdict.
struct ContextResolver<'index> {
    module_edges: BTreeMap<PathBuf, Result<ModuleEdge, &'static str>>,
    include_parents: &'index BTreeMap<PathBuf, ResolvedIncludeParent>,
    memo: BTreeMap<PathBuf, (Context, SourceRoleProvenance)>,
}

impl ContextResolver<'_> {
    fn context_with_provenance(&mut self, file: &Path) -> (Context, SourceRoleProvenance) {
        let mut visiting = BTreeSet::new();
        self.resolve(file, &mut visiting, 0)
    }

    fn resolve(
        &mut self,
        file: &Path,
        visiting: &mut BTreeSet<PathBuf>,
        depth: usize,
    ) -> (Context, SourceRoleProvenance) {
        if let Some(cached) = self.memo.get(file) {
            return cached.clone();
        }
        if depth > MAX_CONTEXT_CHAIN_DEPTH || !visiting.insert(file.to_path_buf()) {
            return (
                Context::Unresolved(REASON_MODULE_CYCLE_OR_DEPTH_LIMIT),
                SourceRoleProvenance {
                    edges: Vec::new(),
                    earliest_unresolved_reason: Some(
                        REASON_MODULE_CYCLE_OR_DEPTH_LIMIT.to_string(),
                    ),
                },
            );
        }

        let module_edge = self.module_edges.get(file).cloned();
        let include_parent = self.include_parents.get(file).cloned();

        let result = match (module_edge, include_parent) {
            (Some(Err(reason)), _) => {
                let context = Context::Unresolved(reason);
                let provenance = SourceRoleProvenance {
                    edges: Vec::new(),
                    earliest_unresolved_reason: context.unresolved_reason(),
                };
                (context, provenance)
            }
            (Some(Ok(edge)), None) => {
                let (parent_context, mut provenance) =
                    self.resolve(&edge.parent, visiting, depth + 1);
                let context = match (edge.requires_test, &parent_context) {
                    (true, _) => Context::RequiresTest,
                    (false, Context::RequiresTest) => Context::RequiresTest,
                    (false, Context::Production) => Context::Production,
                    (false, Context::Unresolved(reason)) => Context::Unresolved(reason),
                };
                provenance.edges.push(SourceRoleProvenanceEdge {
                    kind: SourceRoleProvenanceEdgeKind::Module,
                    parent: edge.parent,
                    child: file.to_path_buf(),
                    declaration: edge.declaration,
                    line: edge.line,
                    requires_test: edge.requires_test,
                });
                if provenance.earliest_unresolved_reason.is_none() {
                    provenance.earliest_unresolved_reason = context.unresolved_reason();
                }
                (context, provenance)
            }
            (None, Some(include)) => {
                let (parent_context, mut provenance) =
                    self.resolve(&include.parent, visiting, depth + 1);
                let parent_unresolved = parent_context.unresolved_reason();
                // A cfg-gated include invocation only exists in test builds,
                // so the fragment's content is test-only regardless of the
                // parent context (#3533 review).
                let context = if include.requires_test {
                    Context::RequiresTest
                } else {
                    parent_context
                };
                let requires_test =
                    include.requires_test || context.requires_test().unwrap_or(false);
                provenance.edges.push(SourceRoleProvenanceEdge {
                    kind: SourceRoleProvenanceEdgeKind::Include,
                    parent: include.parent.clone(),
                    child: file.to_path_buf(),
                    declaration: "include!".to_string(),
                    // The include pass keeps only the parent map, not the
                    // directive line; the edge is exactly identified by the
                    // parent/child pair.
                    line: 0,
                    requires_test,
                });
                // The gate resolves the verdict, but an unresolved edge in
                // the ancestry still happened: keep naming it.
                if let Some(reason) = parent_unresolved
                    && provenance.earliest_unresolved_reason.is_none()
                {
                    provenance.earliest_unresolved_reason = Some(reason);
                }
                (context, provenance)
            }
            (Some(Ok(edge)), Some(include)) => {
                // Dual contextual ownership: compose only when both sides
                // agree on the verdict, naming the conflict otherwise (law 10).
                let (module_context, module_provenance) =
                    self.resolve(&edge.parent, visiting, depth + 1);
                let (include_context, include_provenance) =
                    self.resolve(&include.parent, visiting, depth + 1);
                let module_verdict = if edge.requires_test {
                    Some(true)
                } else {
                    module_context.requires_test()
                };
                // A cfg-gated invocation is test-only on its own (#3533
                // review); an unconditional include inherits its context.
                let include_verdict = if include.requires_test {
                    Some(true)
                } else {
                    include_context.requires_test()
                };
                let mut provenance = module_provenance;
                let mut edges = include_provenance.edges;
                edges.push(SourceRoleProvenanceEdge {
                    kind: SourceRoleProvenanceEdgeKind::Include,
                    parent: include.parent.clone(),
                    child: file.to_path_buf(),
                    declaration: "include!".to_string(),
                    line: 0,
                    requires_test: include_verdict.unwrap_or(false),
                });
                provenance.edges.append(&mut edges);
                match (module_verdict, include_verdict) {
                    (Some(module_test), Some(include_test)) if module_test == include_test => {
                        provenance.edges.push(SourceRoleProvenanceEdge {
                            kind: SourceRoleProvenanceEdgeKind::Module,
                            parent: edge.parent,
                            child: file.to_path_buf(),
                            declaration: edge.declaration,
                            line: edge.line,
                            requires_test: edge.requires_test,
                        });
                        let context = if module_test {
                            Context::RequiresTest
                        } else {
                            Context::Production
                        };
                        // Earliest unresolved wins across BOTH chains: the
                        // include chain's reason must survive the merge.
                        if provenance.earliest_unresolved_reason.is_none() {
                            provenance.earliest_unresolved_reason =
                                include_provenance.earliest_unresolved_reason;
                        }
                        if provenance.earliest_unresolved_reason.is_none() {
                            provenance.earliest_unresolved_reason = context.unresolved_reason();
                        }
                        (context, provenance)
                    }
                    (Some(_), Some(_)) => {
                        provenance.earliest_unresolved_reason =
                            Some(REASON_MODULE_CONTEXT_CONFLICT.to_string());
                        (
                            Context::Unresolved(REASON_MODULE_CONTEXT_CONFLICT),
                            provenance,
                        )
                    }
                    (None, _) | (_, None) => {
                        let reason = module_context
                            .unresolved_reason()
                            .or_else(|| include_context.unresolved_reason())
                            .unwrap_or_else(|| REASON_MODULE_CONTEXT_CONFLICT.to_string());
                        provenance.earliest_unresolved_reason = Some(reason);
                        (
                            Context::Unresolved(REASON_MODULE_CONTEXT_CONFLICT),
                            provenance,
                        )
                    }
                }
            }
            (None, None) => (Context::Production, SourceRoleProvenance::default()),
        };

        visiting.remove(file);
        self.memo.insert(file.to_path_buf(), result.clone());
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::facts::build_index_from_loaded_files_with_cache_and_test_harnesses;
    use crate::analysis::syntax::{RaRustSyntaxAdapter, RustSyntaxAdapter};
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> Result<PathBuf, String> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!("ripr-role-composition-{name}-{stamp}"));
        fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        Ok(dir)
    }

    fn write_manifest(root: &Path) -> Result<(), String> {
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='role-composition'\nversion='0.1.0'\nedition='2024'\n",
        )
        .map_err(|error| error.to_string())
    }

    fn write(root: &Path, relative: &str, source: &str) -> Result<PathBuf, String> {
        let full = root.join(relative);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(&full, source).map_err(|error| error.to_string())?;
        Ok(PathBuf::from(relative))
    }

    fn role_of(index: &RustIndex, file: &str, name: &str) -> Result<FunctionSourceRole, String> {
        index
            .functions
            .iter()
            .find(|function| function.file == Path::new(file) && function.name == name)
            .map(|function| function.source_role)
            .ok_or_else(|| format!("missing function fact for {file}::{name}"))
    }

    fn file_role_of(
        index: &RustIndex,
        file: &str,
        name: &str,
    ) -> Result<FunctionSourceRole, String> {
        index
            .files
            .get(Path::new(file))
            .and_then(|facts| {
                facts
                    .functions
                    .iter()
                    .find(|function| function.name == name)
                    .map(|function| function.source_role)
            })
            .ok_or_else(|| format!("missing per-file fact for {file}::{name}"))
    }

    fn test_names(index: &RustIndex) -> Vec<String> {
        index.tests.iter().map(|test| test.name.clone()).collect()
    }

    /// The live-defect shape: ripr's own `#[cfg(test)] mod tests;` modules.
    /// The normalizer runs before composition and its same-file walk cannot
    /// see the out-of-line gate, so without the composition pass this helper
    /// demotes to `Production` — this test doubles as the
    /// normalizer-does-not-stomp pin (reordering the passes in `facts::mod`
    /// fails it).
    #[test]
    fn out_of_line_cfg_test_module_composes_evidence_roles() -> Result<(), String> {
        let root = temp_dir("out-of-line")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "pub fn production_control() -> i32 { 1 }\n\n#[cfg(test)]\nmod tests;\n",
            )?,
            write(
                &root,
                "src/tests.rs",
                "pub fn unattributed_helper() -> i32 { 2 }\n\n#[test]\nfn real_test() { assert_eq!(unattributed_helper(), 2); }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/lib.rs", "production_control")?,
            FunctionSourceRole::Production
        );
        assert_eq!(
            role_of(&index, "src/tests.rs", "unattributed_helper")?,
            FunctionSourceRole::CfgTestModule
        );
        assert_eq!(
            role_of(&index, "src/tests.rs", "real_test")?,
            FunctionSourceRole::TestAttribute
        );
        assert_eq!(
            file_role_of(&index, "src/tests.rs", "unattributed_helper")?,
            FunctionSourceRole::CfgTestModule,
            "per-file facts and the flat list must stay in sync"
        );
        // Composition only mints evidence-only roles; executable-test
        // membership stays TestFact-driven (law 9).
        let names = test_names(&index);
        assert!(names.contains(&"real_test".to_string()));
        assert!(!names.contains(&"unattributed_helper".to_string()));
        // Provenance records the granting chain.
        let provenance = &index.files[Path::new("src/tests.rs")].role_provenance;
        assert!(provenance.earliest_unresolved_reason.is_none());
        assert_eq!(provenance.edges.len(), 1);
        let edge = &provenance.edges[0];
        assert_eq!(edge.kind, SourceRoleProvenanceEdgeKind::Module);
        assert_eq!(edge.parent, Path::new("src/lib.rs"));
        assert_eq!(edge.child, Path::new("src/tests.rs"));
        assert!(edge.requires_test);
        assert_eq!(edge.declaration, "mod tests;");
        assert!(edge.line >= 3);
        // Standalone files record no provenance.
        let parent_provenance = &index.files[Path::new("src/lib.rs")].role_provenance;
        assert!(parent_provenance.edges.is_empty());
        Ok(())
    }

    /// Control: a production `mod child;` keeps its child
    /// production-subject eligible (law 2).
    #[test]
    fn production_module_child_control_stays_production() -> Result<(), String> {
        let root = temp_dir("production-child")?;
        write_manifest(&root)?;
        let files = vec![
            write(&root, "src/lib.rs", "mod child;\n")?,
            write(
                &root,
                "src/child.rs",
                "pub fn child_helper() -> i32 { 1 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/child.rs", "child_helper")?,
            FunctionSourceRole::Production
        );
        // The chain is recorded without granting: the edge resolved but does
        // not require a test build.
        let provenance = &index.files[Path::new("src/child.rs")].role_provenance;
        assert!(provenance.earliest_unresolved_reason.is_none());
        assert_eq!(provenance.edges.len(), 1);
        assert!(!provenance.edges[0].requires_test);
        Ok(())
    }

    /// The declaration chain composes transitively: a test-required module
    /// under an ordinary production parent still gates its own child file.
    #[test]
    fn nested_chain_through_production_parent_composes() -> Result<(), String> {
        let root = temp_dir("nested-chain")?;
        write_manifest(&root)?;
        let files = vec![
            write(&root, "src/lib.rs", "mod ordinary;\n")?,
            write(
                &root,
                "src/ordinary.rs",
                "pub fn ordinary_helper() -> i32 { 1 }\n\n#[cfg(test)]\nmod tests;\n",
            )?,
            write(
                &root,
                "src/ordinary/tests.rs",
                "pub fn nested_helper() -> i32 { 2 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/ordinary.rs", "ordinary_helper")?,
            FunctionSourceRole::Production
        );
        assert_eq!(
            role_of(&index, "src/ordinary/tests.rs", "nested_helper")?,
            FunctionSourceRole::CfgTestModule
        );
        let provenance = &index.files[Path::new("src/ordinary/tests.rs")].role_provenance;
        assert!(provenance.earliest_unresolved_reason.is_none());
        let edges = &provenance.edges;
        assert_eq!(edges.len(), 2, "both chain edges are recorded");
        assert_eq!(edges[0].parent, Path::new("src/lib.rs"));
        assert_eq!(edges[0].child, Path::new("src/ordinary.rs"));
        assert!(!edges[0].requires_test, "the outer parent is production");
        assert_eq!(edges[1].parent, Path::new("src/ordinary.rs"));
        assert_eq!(edges[1].child, Path::new("src/ordinary/tests.rs"));
        assert!(edges[1].requires_test);
        Ok(())
    }

    /// An exact `#[path]` redirection follows the declaring module's cfg
    /// context (law 5).
    #[test]
    fn exact_path_attribute_module_follows_declaring_context() -> Result<(), String> {
        let root = temp_dir("path-attribute")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "#[cfg(test)]\n#[path = \"support/shared.rs\"]\nmod support;\n",
            )?,
            write(
                &root,
                "src/support/shared.rs",
                "pub fn redirected_helper() -> i32 { 1 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/support/shared.rs", "redirected_helper")?,
            FunctionSourceRole::CfgTestModule
        );
        Ok(())
    }

    #[test]
    fn path_selected_parent_resolves_default_children_from_its_directory() -> Result<(), String> {
        let root = temp_dir("path-selected-child")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "#[cfg(test)] #[path = \"nested/renamed.rs\"] mod nested;",
            )?,
            write(&root, "src/nested/renamed.rs", "mod child;")?,
            write(&root, "src/nested/child.rs", "pub fn correct_child() {}")?,
            write(
                &root,
                "src/nested/renamed/child.rs",
                "pub fn wrong_child() {}",
            )?,
        ];
        let index = crate::analysis::facts::build_index(&root, &files)?;
        assert_eq!(
            role_of(&index, "src/nested/child.rs", "correct_child")?,
            FunctionSourceRole::CfgTestModule
        );
        assert_eq!(
            role_of(&index, "src/nested/renamed/child.rs", "wrong_child")?,
            FunctionSourceRole::Production
        );
        Ok(())
    }

    #[test]
    fn included_fragment_resolves_default_children_from_its_physical_directory()
    -> Result<(), String> {
        let root = temp_dir("included-default-child")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "#[cfg(test)] include!(\"fragments/body.rs\");",
            )?,
            write(&root, "src/fragments/body.rs", "mod child;")?,
            write(&root, "src/fragments/child.rs", "pub fn correct_child() {}")?,
            write(&root, "src/child.rs", "pub fn wrong_child() {}")?,
        ];
        let index = crate::analysis::facts::build_index(&root, &files)?;
        assert_eq!(
            role_of(&index, "src/fragments/child.rs", "correct_child")?,
            FunctionSourceRole::CfgTestModule
        );
        assert_eq!(
            role_of(&index, "src/child.rs", "wrong_child")?,
            FunctionSourceRole::Production
        );
        Ok(())
    }

    fn check_mixed_directory_children(label: &str, root_source: &str) -> Result<(), String> {
        let root = temp_dir(label)?;
        write_manifest(&root)?;
        let files = vec![
            write(&root, "src/lib.rs", root_source)?,
            write(&root, "src/alias.rs", "mod child;")?,
            write(&root, "src/alias/child.rs", "pub fn ordinary_child() {}")?,
            write(&root, "src/child.rs", "pub fn containing_child() {}")?,
        ];
        let index = crate::analysis::facts::build_index(&root, &files)?;
        check_ambiguous_children(
            &index,
            &[
                ("src/alias/child.rs", "ordinary_child"),
                ("src/child.rs", "containing_child"),
            ],
        )
    }

    fn check_ambiguous_children(
        index: &RustIndex,
        children: &[(&str, &str)],
    ) -> Result<(), String> {
        for &(file, function) in children {
            if role_of(index, file, function)? != FunctionSourceRole::Production {
                return Err(format!("ambiguous child {file} received an evidence role"));
            }
            let facts = index
                .files
                .get(Path::new(file))
                .ok_or_else(|| format!("missing child facts for {file}"))?;
            if facts.role_provenance.earliest_unresolved_reason.as_deref()
                != Some(REASON_MODULE_AMBIGUOUS_PARENT)
            {
                return Err(format!(
                    "both directory occurrences must stay ambiguous: {file}: {:?}",
                    facts.role_provenance.earliest_unresolved_reason
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn ordinary_and_path_occurrences_keep_both_children_ambiguous() -> Result<(), String> {
        check_mixed_directory_children(
            "ordinary-and-path",
            "#[cfg(test)] mod alias;\n#[cfg(test)] #[path = \"alias.rs\"] mod renamed;",
        )
    }

    #[test]
    fn ordinary_and_include_occurrences_keep_both_children_ambiguous() -> Result<(), String> {
        check_mixed_directory_children(
            "ordinary-and-include",
            "mod alias;\n#[cfg(test)] include!(\"alias.rs\");",
        )
    }

    #[test]
    fn mixed_directory_anchors_do_not_select_the_only_indexed_layout() -> Result<(), String> {
        let root = temp_dir("mixed-anchor-missing-layout")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "#[cfg(test)] mod alias;\n#[cfg(test)] include!(\"alias.rs\");",
            )?,
            write(&root, "src/alias.rs", "mod child;")?,
            write(&root, "src/child.rs", "pub fn containing_child() {}")?,
        ];
        let index = crate::analysis::facts::build_index(&root, &files)?;
        check_ambiguous_children(&index, &[("src/child.rs", "containing_child")])
    }

    #[test]
    fn mixed_directory_anchors_propagate_through_default_children() -> Result<(), String> {
        let root = temp_dir("mixed-anchor-grandchildren")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "#[cfg(test)] mod outer;\n\
                 #[cfg(test)] #[path = \"outer.rs\"] mod renamed;\n\
                 #[cfg(test)] #[path = \"outer/alias.rs\"] mod alias_path;",
            )?,
            write(&root, "src/outer.rs", "mod alias;")?,
            write(&root, "src/alias.rs", "pub fn other_layout() {}")?,
            write(&root, "src/outer/alias.rs", "mod child;")?,
            write(
                &root,
                "src/outer/alias/child.rs",
                "pub fn ordinary_child() {}",
            )?,
            write(&root, "src/outer/child.rs", "pub fn containing_child() {}")?,
        ];
        let index = crate::analysis::facts::build_index(&root, &files)?;
        check_ambiguous_children(
            &index,
            &[
                ("src/outer/alias/child.rs", "ordinary_child"),
                ("src/outer/child.rs", "containing_child"),
            ],
        )
    }

    /// A dynamic `#[path]` expression must not fall back to default name
    /// resolution: the typed unknown fails closed (law 6).
    #[test]
    fn dynamic_path_attribute_fails_closed() -> Result<(), String> {
        let root = temp_dir("dynamic-path")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "#[cfg(test)]\n#[path = concat!(env!(\"OUT_DIR\"), \"generated.rs\")]\nmod generated;\n",
            )?,
            write(
                &root,
                "src/generated.rs",
                "pub fn generated_helper() -> i32 { 1 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/generated.rs", "generated_helper")?,
            FunctionSourceRole::Production,
            "an unresolvable #[path] must not compose a role, even though a same-named file exists at the default location"
        );
        Ok(())
    }

    /// A file-level include fragment inherits the including context: under a
    /// test-required module the fragment is evidence-role, and its own exact
    /// test attributes still win (evidence union).
    #[test]
    fn include_fragment_inherits_test_required_context() -> Result<(), String> {
        let root = temp_dir("include-test-context")?;
        write_manifest(&root)?;
        let files = vec![
            write(&root, "src/lib.rs", "#[cfg(test)]\nmod tests;\n")?,
            write(&root, "src/tests.rs", "include!(\"fragment.rs\");\n")?,
            write(
                &root,
                "src/fragment.rs",
                "pub fn fragment_helper() -> i32 { 3 }\n\n#[test]\nfn fragment_test() { assert_eq!(fragment_helper(), 3); }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/fragment.rs", "fragment_helper")?,
            FunctionSourceRole::CfgTestModule
        );
        assert_eq!(
            role_of(&index, "src/fragment.rs", "fragment_test")?,
            FunctionSourceRole::TestAttribute
        );
        let names = test_names(&index);
        assert!(names.contains(&"fragment_test".to_string()));
        assert!(!names.contains(&"fragment_helper".to_string()));
        let provenance = &index.files[Path::new("src/fragment.rs")].role_provenance;
        assert!(provenance.earliest_unresolved_reason.is_none());
        let edges = &provenance.edges;
        assert_eq!(edges.len(), 2, "module edge then include edge");
        assert_eq!(edges[0].kind, SourceRoleProvenanceEdgeKind::Module);
        assert_eq!(edges[1].kind, SourceRoleProvenanceEdgeKind::Include);
        assert_eq!(edges[1].parent, Path::new("src/tests.rs"));
        assert_eq!(edges[1].child, Path::new("src/fragment.rs"));
        assert!(edges[1].requires_test);
        Ok(())
    }

    /// Both sides production stays production: a fragment included by a
    /// production compilation unit keeps production-subject eligibility
    /// (law 4, second half).
    #[test]
    fn production_include_requires_both_sides_production() -> Result<(), String> {
        let root = temp_dir("include-production")?;
        write_manifest(&root)?;
        let files = vec![
            write(&root, "src/lib.rs", "include!(\"shared.rs\");\n")?,
            write(
                &root,
                "src/shared.rs",
                "pub fn shared_helper() -> i32 { 1 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/shared.rs", "shared_helper")?,
            FunctionSourceRole::Production
        );
        Ok(())
    }

    /// A `#[cfg(test)] include!(...)` invocation only exists in test builds,
    /// so the fragment's content is test-only regardless of the including
    /// file's own context (#3533 review). Without composing the invocation's
    /// gate, the fragment's helpers re-enter the production inventory.
    #[test]
    fn cfg_gated_include_composes_test_context() -> Result<(), String> {
        let root = temp_dir("gated-include")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "pub fn production_control() -> i32 { 1 }\n\n#[cfg(test)]\ninclude!(\"fragment.rs\");\n",
            )?,
            write(
                &root,
                "src/fragment.rs",
                "pub fn gated_fragment_helper() -> i32 { 3 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/lib.rs", "production_control")?,
            FunctionSourceRole::Production
        );
        assert_eq!(
            role_of(&index, "src/fragment.rs", "gated_fragment_helper")?,
            FunctionSourceRole::CfgTestModule,
            "the fragment of a test-gated invocation is test-only even beside a production unit"
        );
        let provenance = &index.files[Path::new("src/fragment.rs")].role_provenance;
        assert!(provenance.earliest_unresolved_reason.is_none());
        assert_eq!(provenance.edges.len(), 1);
        assert_eq!(
            provenance.edges[0].kind,
            SourceRoleProvenanceEdgeKind::Include
        );
        assert_eq!(provenance.edges[0].parent, Path::new("src/lib.rs"));
        assert!(
            provenance.edges[0].requires_test,
            "the include edge records the invocation's test requirement"
        );
        Ok(())
    }

    /// One file whose include invocations disagree on the cfg-test
    /// requirement (`include!` next to `#[cfg(test)] include!` of the same
    /// fragment) resolves to no single context: the child keeps its
    /// standalone roles and the conflict is named (#3533).
    #[test]
    fn conflicting_include_requirements_fail_closed() -> Result<(), String> {
        let root = temp_dir("conflicting-include")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "include!(\"fragment.rs\");\n\n#[cfg(test)]\ninclude!(\"fragment.rs\");\n",
            )?,
            write(
                &root,
                "src/fragment.rs",
                "pub fn contested_fragment_helper() -> i32 { 1 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert!(
            !index
                .include_parents
                .contains_key(Path::new("src/fragment.rs")),
            "no single context exists for the contested fragment"
        );
        assert!(
            index
                .include_limitations
                .iter()
                .any(|limitation| limitation.reason_code
                    == "rust_include_conflicting_cfg_requirement"),
            "the conflict must be named on the include limitations"
        );
        assert_eq!(
            role_of(&index, "src/fragment.rs", "contested_fragment_helper")?,
            FunctionSourceRole::Production,
            "the contested fragment keeps its standalone roles"
        );
        Ok(())
    }

    /// A conflicted include still has a physical source location for default
    /// module lookup. The conflict must suppress contextual grants, while
    /// `mod child;` in the fragment resolves beside the fragment (`src`), not
    /// under the fragment's stem directory (`src/fragment`).
    #[test]
    fn conflicted_include_preserves_physical_anchor_without_role_grant() -> Result<(), String> {
        let root = temp_dir("conflicting-include-anchor")?;
        write_manifest(&root)?;
        let mut files = vec![
            write(
                &root,
                "src/lib.rs",
                "include!(\"fragment.rs\");\n\n#[cfg(test)]\ninclude!(\"fragment.rs\");\n",
            )?,
            write(
                &root,
                "src/fragment.rs",
                "#[cfg(test)]\nmod child;\npub fn fragment_helper() -> i32 { 3 }\n",
            )?,
            write(
                &root,
                "src/child.rs",
                "pub fn physically_anchored_child() -> i32 { 1 }\n",
            )?,
            write(
                &root,
                "src/fragment/child.rs",
                "pub fn misleading_stem_child() -> i32 { 2 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert!(
            index.include_parents.is_empty(),
            "conflicting cfg requirements must retain fail-closed contextual ownership"
        );
        assert!(
            index.include_targets.contains(Path::new("src/fragment.rs")),
            "physical include discovery must survive contextual conflict"
        );
        assert_eq!(
            role_of(&index, "src/fragment.rs", "fragment_helper")?,
            FunctionSourceRole::Production,
            "physical discovery must not resolve the fragment's contextual role conflict"
        );
        assert!(
            index
                .files
                .get(Path::new("src/child.rs"))
                .ok_or("physical child facts missing")?
                .role_provenance
                .edges
                .iter()
                .any(|edge| edge.kind == SourceRoleProvenanceEdgeKind::Module
                    && edge.parent == Path::new("src/fragment.rs")),
            "default module must resolve beside the physically included fragment"
        );
        assert!(
            index
                .files
                .get(Path::new("src/fragment/child.rs"))
                .ok_or("stem child facts missing")?
                .role_provenance
                .edges
                .is_empty(),
            "the fragment stem directory must not be selected as the anchor"
        );
        assert_eq!(
            role_of(&index, "src/child.rs", "physically_anchored_child")?,
            FunctionSourceRole::CfgTestModule,
            "the module's own cfg(test) edge grants its child without resolving the conflicting include"
        );
        assert_eq!(
            role_of(&index, "src/fragment/child.rs", "misleading_stem_child")?,
            FunctionSourceRole::Production,
            "the misleading stem child must not gain an evidence role"
        );

        // Rust does not fall back to the stem layout when the physical child
        // is absent. Keep the misleading candidate in the full source index.
        let physical_child = root.join("src/child.rs");
        fs::remove_file(&physical_child).map_err(|error| error.to_string())?;
        files.retain(|file| file != Path::new("src/child.rs"));
        let missing = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;
        assert_eq!(
            role_of(&missing, "src/fragment/child.rs", "misleading_stem_child")?,
            FunctionSourceRole::Production,
            "a missing physical child must not grant the stem candidate a role"
        );
        assert!(
            missing
                .files
                .get(Path::new("src/fragment/child.rs"))
                .ok_or("stem child facts missing after physical child removal")?
                .role_provenance
                .edges
                .is_empty(),
            "a missing physical child must not create a stem-directory edge"
        );
        Ok(())
    }

    /// Two parent declarations claiming the same child file fail closed and
    /// name the edge; the child keeps its standalone roles.
    #[test]
    fn ambiguous_module_parents_fail_closed() -> Result<(), String> {
        let root = temp_dir("ambiguous-module")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "mod other;\n\n#[path = \"shared.rs\"]\nmod from_lib;\n",
            )?,
            write(
                &root,
                "src/other.rs",
                "#[path = \"shared.rs\"]\nmod from_other;\n",
            )?,
            write(
                &root,
                "src/shared.rs",
                "pub fn contested_helper() -> i32 { 1 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/shared.rs", "contested_helper")?,
            FunctionSourceRole::Production
        );
        let provenance = &index.files[Path::new("src/shared.rs")].role_provenance;
        assert_eq!(
            provenance.earliest_unresolved_reason.as_deref(),
            Some(REASON_MODULE_AMBIGUOUS_PARENT),
            "the earliest unresolved edge must be named"
        );
        Ok(())
    }

    /// One file both declared as a module and included as a fragment, with
    /// the two contexts disagreeing on test requirement, fails closed
    /// (law 10: duplicate contextual ownership names the edge).
    #[test]
    fn module_include_context_conflict_fails_closed() -> Result<(), String> {
        let root = temp_dir("context-conflict")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "mod shared;\n\n#[cfg(test)]\nmod tests;\n",
            )?,
            write(&root, "src/tests.rs", "include!(\"shared.rs\");\n")?,
            write(
                &root,
                "src/shared.rs",
                "pub fn dual_context_helper() -> i32 { 1 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/shared.rs", "dual_context_helper")?,
            FunctionSourceRole::Production
        );
        let provenance = &index.files[Path::new("src/shared.rs")].role_provenance;
        assert_eq!(
            provenance.earliest_unresolved_reason.as_deref(),
            Some(REASON_MODULE_CONTEXT_CONFLICT)
        );
        Ok(())
    }

    /// The agreeing dual-context shape composes: both contexts require a
    /// test build, so the evidence grant is sound.
    #[test]
    fn agreeing_dual_context_composes() -> Result<(), String> {
        let root = temp_dir("dual-agree")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "#[cfg(test)]\nmod shared;\n\n#[cfg(test)]\nmod tests;\n",
            )?,
            write(&root, "src/tests.rs", "include!(\"shared.rs\");\n")?,
            write(
                &root,
                "src/shared.rs",
                "pub fn agreed_helper() -> i32 { 1 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/shared.rs", "agreed_helper")?,
            FunctionSourceRole::CfgTestModule
        );
        let provenance = &index.files[Path::new("src/shared.rs")].role_provenance;
        assert!(provenance.earliest_unresolved_reason.is_none());
        assert!(
            provenance.edges.len() >= 3,
            "include chain plus module edge are all recorded"
        );
        Ok(())
    }

    /// Composed roles survive the warm per-file fact cache: cached entries
    /// carry the module-declaration producer output (FILE_FACT cache schema
    /// 0.8), and the composition pass re-derives roles on warm runs.
    #[test]
    fn composed_roles_survive_warm_file_fact_cache() -> Result<(), String> {
        let root = temp_dir("warm-cache")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "pub fn production_control() -> i32 { 1 }\n\n#[cfg(test)]\nmod tests;\n",
            )?,
            write(
                &root,
                "src/tests.rs",
                "pub fn unattributed_helper() -> i32 { 2 }\n",
            )?,
        ];
        let mut loaded: Vec<(PathBuf, Vec<u8>)> = Vec::new();
        for file in &files {
            let bytes = fs::read(root.join(file)).map_err(|error| error.to_string())?;
            loaded.push((file.clone(), bytes));
        }

        let cold = build_index_from_loaded_files_with_cache_and_test_harnesses(&root, &loaded, &[])
            .map_err(|error| error.to_string())?;
        assert_eq!(
            role_of(&cold.index, "src/tests.rs", "unattributed_helper")?,
            FunctionSourceRole::CfgTestModule
        );

        let warm = build_index_from_loaded_files_with_cache_and_test_harnesses(&root, &loaded, &[])
            .map_err(|error| error.to_string())?;
        assert!(
            warm.file_fact_cache.hits > 0,
            "the warm run must hit the per-file fact cache"
        );
        assert_eq!(
            role_of(&warm.index, "src/tests.rs", "unattributed_helper")?,
            FunctionSourceRole::CfgTestModule,
            "a warm cache hit must not serve composition-blind facts"
        );
        Ok(())
    }

    /// A `path` attribute introduced conditionally by `cfg_attr` fails
    /// closed (#3533 review): which file the compiler loads depends on the
    /// active configuration, so neither the conditional target nor the
    /// default layout may gain a composed evidence role.
    #[test]
    fn cfg_attr_introduced_path_target_fails_closed() -> Result<(), String> {
        let root = temp_dir("cfg-attr-path")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "#[cfg(test)]\n#[cfg_attr(test, path = \"test_impl.rs\")]\nmod imp;\n",
            )?,
            write(
                &root,
                "src/imp.rs",
                "pub fn default_layout_helper() -> i32 { 1 }\n",
            )?,
            write(
                &root,
                "src/test_impl.rs",
                "pub fn conditional_target_helper() -> i32 { 2 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/imp.rs", "default_layout_helper")?,
            FunctionSourceRole::Production,
            "the default file is not compiled under the conditional configuration and must not compose a role"
        );
        assert_eq!(
            role_of(&index, "src/test_impl.rs", "conditional_target_helper")?,
            FunctionSourceRole::Production,
            "the conditional target is not statically resolvable and must not be credited either"
        );
        // The producer classifies the declaration as a typed unknown.
        let facts = RaRustSyntaxAdapter
            .summarize_file(
                Path::new("src/lib.rs"),
                &std::fs::read_to_string(root.join("src/lib.rs"))
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
        assert_eq!(facts.module_declarations.len(), 1);
        assert_eq!(
            facts.module_declarations[0].path_target,
            ModulePathTarget::Unknown
        );
        assert!(facts.module_declarations[0].requires_test);
        Ok(())
    }

    /// A Unicode-identifier `cfg_attr` condition is valid Rust the bounded
    /// ASCII lexer cannot read (#3533 review). Treating that lexer failure as
    /// "not a conditional path" would fall back to default name resolution
    /// and grant an unrelated default-layout file the composed evidence role;
    /// the unreadable `cfg_attr` must fail closed to a typed unknown instead.
    #[test]
    fn unlexable_unicode_cfg_attr_path_fails_closed() -> Result<(), String> {
        let root = temp_dir("unicode-cfg-attr-path")?;
        write_manifest(&root)?;
        let files = vec![
            write(
                &root,
                "src/lib.rs",
                "#[cfg(test)]\n#[cfg_attr(é, path = \"alternate.rs\")]\nmod imp;\n",
            )?,
            write(
                &root,
                "src/imp.rs",
                "pub fn default_layout_helper() -> i32 { 1 }\n",
            )?,
            write(
                &root,
                "src/alternate.rs",
                "pub fn conditional_target_helper() -> i32 { 2 }\n",
            )?,
            // Non-cfg_attr Unicode control: an unlexable attribute that
            // cannot introduce a path must keep default resolution working
            // (fail closed only for cfg_attr heads, not for every unlexable
            // attribute).
            write(
                &root,
                "src/controls.rs",
                "#[cfg(test)]\n#[allow(é)]\nmod control;\n",
            )?,
            write(
                &root,
                "src/controls/control.rs",
                "pub fn control_helper() -> i32 { 3 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/imp.rs", "default_layout_helper")?,
            FunctionSourceRole::Production,
            "the unreadable cfg_attr must not resolve the default-layout file as the module child"
        );
        assert_eq!(
            role_of(&index, "src/alternate.rs", "conditional_target_helper")?,
            FunctionSourceRole::Production,
            "the conditional target is not statically resolvable either"
        );
        assert_eq!(
            role_of(&index, "src/controls/control.rs", "control_helper")?,
            FunctionSourceRole::CfgTestModule,
            "Unicode in a non-cfg_attr attribute must not block default resolution"
        );
        // The producer classifies the declaration as a typed unknown even
        // though its condition cannot be tokenized.
        let facts = RaRustSyntaxAdapter
            .summarize_file(
                Path::new("src/lib.rs"),
                &std::fs::read_to_string(root.join("src/lib.rs"))
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|error| error.to_string())?;
        assert_eq!(facts.module_declarations.len(), 1);
        assert_eq!(
            facts.module_declarations[0].path_target,
            ModulePathTarget::Unknown
        );
        Ok(())
    }

    /// A literal `#[path]` inside an included fragment resolves relative to
    /// the fragment file's directory (#3533 review) — the Rust rule for
    /// non-inline `#[path]` targets. Anchoring at the compilation unit
    /// instead would grant the evidence role to a same-named file beside the
    /// including file that Rust never compiles as this module.
    #[test]
    fn included_fragment_path_attribute_resolves_from_the_fragment() -> Result<(), String> {
        let root = temp_dir("fragment-path")?;
        write_manifest(&root)?;
        let files = vec![
            write(&root, "src/lib.rs", "include!(\"frags/frag.rs\");\n")?,
            write(
                &root,
                "src/frags/frag.rs",
                "#[cfg(test)]\n#[path = \"child.rs\"]\nmod child;\n",
            )?,
            write(
                &root,
                "src/frags/child.rs",
                "pub fn fragment_child_helper() -> i32 { 1 }\n",
            )?,
            write(
                &root,
                "src/child.rs",
                "pub fn unit_side_helper() -> i32 { 2 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "src/frags/child.rs", "fragment_child_helper")?,
            FunctionSourceRole::CfgTestModule,
            "the fragment-relative target inherits the include chain's context"
        );
        assert_eq!(
            role_of(&index, "src/child.rs", "unit_side_helper")?,
            FunctionSourceRole::Production,
            "the same-named file beside the compilation unit is not this module"
        );
        let provenance = &index.files[Path::new("src/frags/child.rs")].role_provenance;
        assert!(provenance.earliest_unresolved_reason.is_none());
        let edges = &provenance.edges;
        assert_eq!(
            edges.len(),
            2,
            "the include chain edge then the module edge"
        );
        assert_eq!(edges[0].kind, SourceRoleProvenanceEdgeKind::Include);
        assert_eq!(edges[0].parent, Path::new("src/lib.rs"));
        assert_eq!(edges[1].kind, SourceRoleProvenanceEdgeKind::Module);
        assert_eq!(edges[1].parent, Path::new("src/frags/frag.rs"));
        assert_eq!(edges[1].child, Path::new("src/frags/child.rs"));
        assert!(edges[1].requires_test);
        Ok(())
    }

    /// A custom-named crate root (`[lib] path = "source/root.rs"`) resolves
    /// its out-of-line modules relative to its own containing directory
    /// (#3533 review), not under a directory named after its stem.
    #[test]
    fn custom_crate_root_resolves_modules_relative_to_its_directory() -> Result<(), String> {
        let root = temp_dir("custom-crate-root")?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='role-composition'\nversion='0.1.0'\nedition='2024'\n\n[lib]\npath = \"source/root.rs\"\n",
        )
        .map_err(|error| error.to_string())?;
        let files = vec![
            write(
                &root,
                "source/root.rs",
                "pub fn root_helper() -> i32 { 1 }\n\n#[cfg(test)]\nmod tests;\n",
            )?,
            write(
                &root,
                "source/tests.rs",
                "pub fn unattributed_helper() -> i32 { 2 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "source/root.rs", "root_helper")?,
            FunctionSourceRole::Production
        );
        assert_eq!(
            role_of(&index, "source/tests.rs", "unattributed_helper")?,
            FunctionSourceRole::CfgTestModule,
            "the crate root's sibling tests.rs is the real module child"
        );
        let provenance = &index.files[Path::new("source/tests.rs")].role_provenance;
        assert!(provenance.earliest_unresolved_reason.is_none());
        assert_eq!(provenance.edges.len(), 1);
        assert_eq!(provenance.edges[0].parent, Path::new("source/root.rs"));
        assert_eq!(provenance.edges[0].child, Path::new("source/tests.rs"));
        assert!(provenance.edges[0].requires_test);
        Ok(())
    }

    /// Layout-autodiscovered target files are crate roots too: one file
    /// directly under `tests/`, `benches/`, `examples/`, or `src/bin/`
    /// resolves its modules from its own directory. Controls pin the
    /// stem-directory rule for ordinary module files.
    #[test]
    fn layout_autodiscovered_targets_are_crate_roots() -> Result<(), String> {
        let root = temp_dir("layout-roots")?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='role-composition'\nversion='0.1.0'\nedition='2024'\n",
        )
        .map_err(|error| error.to_string())?;
        let mut crate_roots = CrateRoots::default();
        for file in [
            "tests/pricing.rs",
            "benches/perf.rs",
            "examples/demo.rs",
            "src/bin/tool.rs",
        ] {
            assert!(
                crate_roots.is_crate_root(&root, Path::new(file)),
                "{file} is an autodiscovered crate root"
            );
            assert_eq!(
                directory_of(Path::new(file)),
                Path::new(file)
                    .parent()
                    .map(Path::to_path_buf)
                    .unwrap_or_default()
            );
        }
        // Controls: ordinary module files keep the stem rule, and a file
        // with no ancestor manifest keeps the status-quo stem rule too.
        assert!(!crate_roots.is_crate_root(&root, Path::new("src/foo.rs")));
        assert!(!crate_roots.is_crate_root(&root, Path::new("tests/nested/inner.rs")));
        let orphan_root = temp_dir("layout-roots-orphan")?;
        assert!(!crate_roots.is_crate_root(&orphan_root, Path::new("loose/foo.rs")));
        Ok(())
    }

    /// Sibling files in one directory share the crate-root verdict (#3533
    /// review): the manifest walk must memoize the resolved ancestor answer,
    /// not a per-directory `None`, or the second sibling's walk would stop
    /// early and keep the stem rule — making the verdict depend on which
    /// sibling was resolved first.
    #[test]
    fn sibling_files_share_the_resolved_manifest_answer() -> Result<(), String> {
        let root = temp_dir("sibling-roots")?;
        write_manifest(&root)?;
        // Unit level: the first lookup warms the cache, the second must not
        // stop at the cached intermediate directory.
        let mut crate_roots = CrateRoots::default();
        assert!(crate_roots.is_crate_root(&root, Path::new("tests/a.rs")));
        assert!(
            crate_roots.is_crate_root(&root, Path::new("tests/b.rs")),
            "the second sibling in the same directory is a crate root too"
        );

        // End to end: tests/b.rs declares an out-of-line module and must
        // resolve it from tests/ (crate-root rule), not from tests/b/.
        let files = vec![
            write(&root, "tests/a.rs", "pub fn a_helper() -> i32 { 1 }\n")?,
            write(
                &root,
                "tests/b.rs",
                "pub fn b_helper() -> i32 { 2 }\n\n#[cfg(test)]\nmod helpers;\n",
            )?,
            write(
                &root,
                "tests/helpers.rs",
                "pub fn sibling_child_helper() -> i32 { 3 }\n",
            )?,
        ];

        let index = crate::analysis::facts::build_index(&root, &files)
            .map_err(|error| error.to_string())?;

        assert_eq!(
            role_of(&index, "tests/helpers.rs", "sibling_child_helper")?,
            FunctionSourceRole::CfgTestModule,
            "the second sibling's out-of-line module child composes its evidence role"
        );
        Ok(())
    }
}

#[cfg(test)]
mod cycle_depth_tests {
    use super::*;

    fn temp_dir(name: &str) -> Result<PathBuf, String> {
        let root = std::env::temp_dir().join(format!(
            "role-cycle-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(root.join("src"))
            .map_err(|error| format!("create {}: {error}", root.display()))?;
        Ok(root)
    }

    // XnBs (review): the cycle guard at resolve() must stay exercised —
    // mutual #[path] declarations keep Production roles and name the cycle
    // reason on the provenance chain.
    #[test]
    fn mutual_module_path_cycle_fails_closed_with_cycle_reason() -> Result<(), String> {
        let root = temp_dir("role-cycle-mutual")?;
        std::fs::write(
            root.join("src/a.rs"),
            "#[path = \"b.rs\"]\nmod b;\n\npub fn a_helper() -> i32 { 1 }\n",
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(
            root.join("src/b.rs"),
            "#[path = \"a.rs\"]\nmod a;\n\npub fn b_helper() -> i32 { 2 }\n",
        )
        .map_err(|error| error.to_string())?;
        std::fs::write(root.join("src/lib.rs"), "#[path = \"a.rs\"]\nmod a;\n")
            .map_err(|error| error.to_string())?;

        let index = crate::analysis::facts::build_index(
            &root,
            &[
                PathBuf::from("src/a.rs"),
                PathBuf::from("src/b.rs"),
                PathBuf::from("src/lib.rs"),
            ],
        )
        .map_err(|error| error.to_string())?;

        // The mutual pair resolves as ambiguous parents (both declarations
        // reach both files) — fail-closed before any recursion.
        for (file, helper) in [("src/a.rs", "a_helper"), ("src/b.rs", "b_helper")] {
            let facts = index
                .files
                .get(Path::new(file))
                .ok_or_else(|| format!("expected {file} in the index"))?;
            for function in &facts.functions {
                if function.name == helper {
                    assert_eq!(
                        function.source_role,
                        FunctionSourceRole::Production,
                        "mutual cycle must keep {helper} production"
                    );
                }
            }
            let provenance = &index.files[Path::new(file)].role_provenance;
            assert_eq!(
                provenance.earliest_unresolved_reason.as_deref(),
                Some(REASON_MODULE_AMBIGUOUS_PARENT),
                "{file} must name the ambiguity reason"
            );
        }
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn self_referential_module_path_names_the_cycle_reason() -> Result<(), String> {
        let root = temp_dir("role-cycle-self")?;
        std::fs::write(
            root.join("src/lib.rs"),
            "#[path = \"lib.rs\"]
mod self_cycle;

pub fn helper() -> i32 { 1 }
",
        )
        .map_err(|error| error.to_string())?;

        let index = crate::analysis::facts::build_index(&root, &[PathBuf::from("src/lib.rs")])
            .map_err(|error| error.to_string())?;

        let provenance = &index.files[Path::new("src/lib.rs")].role_provenance;
        assert_eq!(
            provenance.earliest_unresolved_reason.as_deref(),
            Some(REASON_MODULE_CYCLE_OR_DEPTH_LIMIT),
            "a self-referential module path must name the cycle reason"
        );
        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }
}
