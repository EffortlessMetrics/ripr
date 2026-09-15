//! Forward/reverse path-dependency adjacency (#2969, slice B of #2665).
//!
//! Built deterministically in memory from the path-dependency edges captured
//! by slice A (#2968/#3037, see
//! [`crate::analysis::seam_cache::PathDependencyEdge`]). No network, no
//! filesystem walking of dependencies, no Cargo invocation: node identities
//! are the captured repo-relative manifest paths and the lexically resolved
//! dependency paths, so the graph answers "which manifests does this manifest
//! reach through path dependencies" (forward) and "which manifests reach this
//! manifest" (reverse) without implying that any registry or external
//! dependency metadata was resolved. External dependency provenance stays
//! `unavailable`.
//!
//! Honesty contract:
//!
//! - all iteration is `BTreeMap`/`BTreeSet`-ordered, so no neighbor list or
//!   walk result depends on hash-map iteration order;
//! - edges without a resolved identity (`UnsupportedAbsolutePath`,
//!   `UnresolvedWorkspaceInheritance`, `InvalidDeclaration`) cannot
//!   participate in the adjacency; that gap is disclosed through
//!   `connected_edge_count` and the status detail, never silently dropped;
//! - a cycle terminates the walk and is disclosed as a cycle marker (the
//!   back-edge target), never recursed into;
//! - a non-empty capture-limitation list means the captured edge inventory is
//!   partial, so the status degrades to `limited` and says so;
//! - when no path dependency was declared at all the status is `complete`
//!   with an empty node set and the detail states that truthfully.
//!
//! Scope boundary: this module builds and exposes the graph, and (#2970
//! slice C) computes the reverse-dependency diff-scope expansion that the
//! Rust adapter feeds into the Draft/Fast package selection. Dep-driven
//! impact analysis beyond scope expansion is still future work.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::analysis::seam_cache::{
    PathDependencySection, WorkspaceGraphProvenance, workspace_graph_provenance,
};

/// Build status of the path-dependency adjacency, disclosed alongside every
/// rendered neighbor list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PathDependencyGraphStatus {
    /// Every scanned manifest was parsed and no capture limitation exists, so
    /// the adjacency covers the full captured edge inventory. An empty edge
    /// inventory is also `complete`: there were no path dependencies to walk.
    Complete,
    /// At least one capture limitation was reported, so the captured edge
    /// inventory is partial and the adjacency may be missing edges.
    Limited,
    /// No manifest was found in the scan root, so no adjacency was built.
    Unavailable,
}

impl PathDependencyGraphStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Limited => "limited",
            Self::Unavailable => "unavailable",
        }
    }
}

/// One reachability walk result: the transitively reachable manifests plus
/// the cycle markers cut to keep the walk finite.
///
/// #2970 slice C consumes reverse walks for diff-scope expansion; the
/// forward direction has no production consumer yet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PathDependencyWalk {
    /// Manifest identities reached from the start node, sorted, excluding the
    /// start itself (a start that is re-entered through a cycle is disclosed
    /// in `cycle_manifests` instead of being listed as reached).
    reachable: Vec<String>,
    /// Targets of back-edges to manifests already on the current walk path,
    /// sorted. Non-empty means the walked subgraph is cyclic; the walk
    /// terminated by cutting these edges instead of recursing into them.
    cycle_manifests: Vec<String>,
}

impl PathDependencyWalk {
    /// Manifests reached from the start node, sorted, start excluded.
    pub(crate) fn reachable(&self) -> &[String] {
        &self.reachable
    }

    /// Cycle markers: back-edge targets already on the walk path, sorted.
    #[allow(
        dead_code,
        reason = "cycle markers stay disclosed on the graph surfaces (rerun fingerprint, \
                  adjacency tests); diff-scope expansion needs only the reachable set"
    )]
    pub(crate) fn cycle_manifests(&self) -> &[String] {
        &self.cycle_manifests
    }
}

/// Deterministic forward/reverse adjacency over captured path-dependency
/// edges. Nodes are repo-relative, `/`-separated manifest paths: the
/// declaring manifest, and the target identity formed from the lexically
/// resolved dependency path (a resolved directory `foo` names the manifest
/// `foo/Cargo.toml`; a resolved path that already names a `Cargo.toml` file
/// is kept as-is; the scan root names `Cargo.toml`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PathDependencyAdjacency {
    status: PathDependencyGraphStatus,
    detail: Option<String>,
    edge_count: usize,
    connected_edge_count: usize,
    nodes: BTreeSet<String>,
    forward: BTreeMap<String, BTreeSet<String>>,
    reverse: BTreeMap<String, BTreeSet<String>>,
    /// Captured forward path-dependency declarations per connected
    /// (declarer, target) pair, as `(Cargo section, declared dependency
    /// name)`, so a consumer can require a callable section and read the
    /// name a dependent's `use` paths must start with (#2972 review).
    /// Deduplicated like the neighbor sets: multiple declarations of the
    /// same target collapse to the set of (section, name) declarations.
    forward_declarations: BTreeMap<(String, String), BTreeSet<(PathDependencySection, String)>>,
}

impl PathDependencyAdjacency {
    /// Build the adjacency from the captured path-dependency edges in
    /// `provenance`. Pure: reads no filesystem and no network.
    pub(crate) fn build(provenance: &WorkspaceGraphProvenance) -> Self {
        // `unavailable` is reserved for an absent manifest scan. When the
        // capture still recorded limitations (a partial inventory), the
        // adjacency must disclose them as `limited` instead of taking this
        // shortcut and silently dropping the capture (#3613 review).
        if provenance.package_graph_status == "unavailable"
            && provenance.path_dependency_limitations.is_empty()
        {
            return Self {
                status: PathDependencyGraphStatus::Unavailable,
                detail: Some(
                    "no local Cargo.toml manifest was found; the path-dependency adjacency \
                     was not built"
                        .to_string(),
                ),
                edge_count: 0,
                connected_edge_count: 0,
                nodes: BTreeSet::new(),
                forward: BTreeMap::new(),
                reverse: BTreeMap::new(),
                forward_declarations: BTreeMap::new(),
            };
        }

        let edges = &provenance.path_dependency_edges;
        let mut forward: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut reverse: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut forward_declarations: BTreeMap<
            (String, String),
            BTreeSet<(PathDependencySection, String)>,
        > = BTreeMap::new();
        let mut nodes: BTreeSet<String> = BTreeSet::new();
        let mut connected_edge_count = 0usize;
        for edge in edges {
            // Only `Resolved` edges name a manifest that exists inside the
            // scan root. Outside-root, missing-target, absolute, and
            // invalid declarations stay disconnected: inventing a node for
            // them would fabricate a phantom workspace manifest in a
            // `complete` graph (#3613 review); they remain disclosed in
            // `edge_count` and the detail/limitations. The declaring
            // manifest joins the node set only through a connected edge so
            // a manifest whose every edge is disconnected does not appear
            // as an isolated participant.
            if edge.resolution != crate::analysis::seam_cache::PathDependencyResolution::Resolved {
                continue;
            }
            let Some(resolved) = edge.resolved_path.as_deref() else {
                continue;
            };
            nodes.insert(edge.from_manifest.clone());
            let target = manifest_identity_from_resolved_path(resolved);
            nodes.insert(target.clone());
            forward
                .entry(edge.from_manifest.clone())
                .or_default()
                .insert(target.clone());
            forward_declarations
                .entry((edge.from_manifest.clone(), target.clone()))
                .or_default()
                .insert((edge.section, edge.dependency_name.clone()));
            reverse
                .entry(target)
                .or_default()
                .insert(edge.from_manifest.clone());
            connected_edge_count += 1;
        }

        let edge_count = edges.len();
        let status = if provenance.path_dependency_limitations.is_empty() {
            PathDependencyGraphStatus::Complete
        } else {
            PathDependencyGraphStatus::Limited
        };
        let detail = compose_detail(status, edge_count, connected_edge_count, provenance);
        Self {
            status,
            detail,
            edge_count,
            connected_edge_count,
            nodes,
            forward,
            reverse,
            forward_declarations,
        }
    }

    /// Disclosed build status for this adjacency.
    pub(crate) fn status(&self) -> PathDependencyGraphStatus {
        self.status
    }

    /// Human-readable status detail, present whenever there is a named
    /// boundary to disclose (unavailable scan, empty inventory, partial
    /// inventory, or edges that cannot participate).
    pub(crate) fn detail(&self) -> Option<&str> {
        self.detail.as_deref()
    }

    /// Number of captured edges the adjacency was built from.
    pub(crate) fn edge_count(&self) -> usize {
        self.edge_count
    }

    /// Number of captured edges that carry a resolved identity and therefore
    /// participate in the adjacency. `edge_count - connected_edge_count` is
    /// the disclosed non-participating remainder.
    pub(crate) fn connected_edge_count(&self) -> usize {
        self.connected_edge_count
    }

    /// Every manifest that participates in at least one captured edge (either
    /// side), sorted. A crate declared by no path dependency — isolated from
    /// the path-dep graph — is not a node.
    pub(crate) fn nodes(&self) -> &BTreeSet<String> {
        &self.nodes
    }

    /// Whether `manifest` participates in the graph at all. Unknown manifests
    /// and crates with no path-dependency edges both return `false`.
    #[allow(
        dead_code,
        reason = "adjacency membership probe kept for graph consumers and tests; the scope \
                  expansion resolves membership through the walk itself"
    )]
    pub(crate) fn contains_node(&self, manifest: &str) -> bool {
        self.nodes.contains(manifest)
    }

    /// Direct forward neighbors of `manifest` (the manifests it declares path
    /// dependencies on), sorted. `None` when the manifest has no outgoing
    /// connected edges, including when it is not a graph node.
    pub(crate) fn forward_neighbors(&self, manifest: &str) -> Option<&BTreeSet<String>> {
        self.forward.get(manifest)
    }

    /// Direct reverse neighbors of `manifest` (the manifests that declare a
    /// path dependency on it), sorted. `None` when nothing depends on the
    /// manifest through a connected path edge.
    pub(crate) fn reverse_neighbors(&self, manifest: &str) -> Option<&BTreeSet<String>> {
        self.reverse.get(manifest)
    }

    /// Captured forward path-dependency declarations from `manifest` (the
    /// declarer) to `target` (the dependency), as `(Cargo section, declared
    /// dependency name)`, when at least one connected edge joins them.
    /// `None` when the pair is not joined by any forward connected edge —
    /// including the reverse direction, which is a reverse-side relation
    /// only and is deliberately not reported here (#2972 review: a bare
    /// call in the declarer can only bind names the declarer's own
    /// dependency declarations provide). The declared name is the first
    /// path segment a dependent's `use` paths can start with (an alias
    /// when the declaration renames the package), so import-evidence
    /// consumers need it per declaration, not just the section.
    pub(crate) fn forward_dependency_declarations(
        &self,
        manifest: &str,
        target: &str,
    ) -> Option<&BTreeSet<(PathDependencySection, String)>> {
        self.forward_declarations
            .get(&(manifest.to_string(), target.to_string()))
    }

    /// Transitive forward reach: every manifest `manifest` reaches through
    /// path dependencies, with cycles cut and disclosed. `None` when the
    /// manifest is not a graph node.
    ///
    /// No production consumer yet: the #2970 diff-scope expansion walks the
    /// reverse direction (dependents), and forward scope expansion is not a
    /// stated contract.
    #[allow(
        dead_code,
        reason = "forward direction is pinned by tests and reserved for future dep-driven \
                  consumers; only the reverse walk has a production consumer today"
    )]
    pub(crate) fn forward_walk(&self, manifest: &str) -> Option<PathDependencyWalk> {
        self.walk(&self.forward, manifest)
    }

    /// Transitive reverse reach: every manifest that reaches `manifest`
    /// through path dependencies, with cycles cut and disclosed. `None` when
    /// the manifest is not a graph node.
    pub(crate) fn reverse_walk(&self, manifest: &str) -> Option<PathDependencyWalk> {
        self.walk(&self.reverse, manifest)
    }

    /// Iterative depth-first walk over one adjacency direction. `visited`
    /// expands each node once so shared subgraphs are walked once; a child
    /// already on the current path is a cycle back-edge: disclosed, never
    /// recursed. Together the two guards bound the walk to O(nodes + edges)
    /// steps on any input, cyclic or not.
    fn walk(
        &self,
        adjacency: &BTreeMap<String, BTreeSet<String>>,
        start: &str,
    ) -> Option<PathDependencyWalk> {
        if !self.nodes.contains(start) {
            return None;
        }
        let empty: BTreeSet<String> = BTreeSet::new();
        let mut on_path: BTreeSet<String> = BTreeSet::new();
        on_path.insert(start.to_string());
        let mut visited: BTreeSet<String> = BTreeSet::new();
        let mut reachable: BTreeSet<String> = BTreeSet::new();
        let mut cycles: BTreeSet<String> = BTreeSet::new();
        let mut stack: Vec<(String, std::vec::IntoIter<String>)> = Vec::new();
        let children = adjacency.get(start).unwrap_or(&empty);
        stack.push((
            start.to_string(),
            children.iter().cloned().collect::<Vec<_>>().into_iter(),
        ));
        loop {
            let advance = stack
                .last_mut()
                .and_then(|(_node, children)| children.next());
            match advance {
                Some(child) => {
                    if on_path.contains(&child) {
                        cycles.insert(child);
                        continue;
                    }
                    if visited.contains(&child) {
                        continue;
                    }
                    visited.insert(child.clone());
                    reachable.insert(child.clone());
                    let grandchildren = adjacency
                        .get(&child)
                        .unwrap_or(&empty)
                        .iter()
                        .cloned()
                        .collect::<Vec<_>>()
                        .into_iter();
                    on_path.insert(child.clone());
                    stack.push((child, grandchildren));
                }
                None => match stack.pop() {
                    Some((node, _)) => {
                        on_path.remove(&node);
                    }
                    None => break,
                },
            }
        }
        Some(PathDependencyWalk {
            reachable: reachable.into_iter().collect(),
            cycle_manifests: cycles.into_iter().collect(),
        })
    }
}

/// Reverse-dependency expansion of the diff-scope package set (#2970
/// slice C).
///
/// The diff-scope decision surface (`select_rust_files_for_mode`) narrows
/// Draft/Fast analysis to the packages that own changed files. A behavior
/// change in a path dependency can surface in every crate that depends on
/// it, so the reverse adjacency contributes the dependent package roots to
/// the scope decision. The base scope is never dropped: expansion only ever
/// adds packages, and a graph that could not be built (`unavailable`) or a
/// partial edge inventory (`limited`) is disclosed through
/// [`PathDependencyScopeExpansion::scope_disclosure`] instead of silently
/// narrowing the scope or silently pretending the reach was complete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PathDependencyScopeExpansion {
    status: PathDependencyGraphStatus,
    /// Package roots (forward-slash, trailing `/`, sorted) of manifests that
    /// reach a changed package through the reverse path-dependency adjacency.
    /// Empty when no changed package has dependents, including when the
    /// graph was unavailable.
    dependent_package_roots: BTreeSet<String>,
    /// The discovered manifest directory prefixes behind this expansion, so
    /// consumers attribute unattributed files with the same nearest-manifest
    /// ownership rule the seeding used (#3616 review).
    manifest_dir_prefixes: Vec<String>,
}

impl PathDependencyScopeExpansion {
    /// Discovered manifest directory prefixes (`/`-separated, trailing `/`,
    /// including the empty root prefix), sorted as produced by the walk.
    pub(crate) fn manifest_dir_prefixes(&self) -> &[String] {
        &self.manifest_dir_prefixes
    }
    /// Disclosed graph state behind this expansion.
    #[allow(
        dead_code,
        reason = "status accessor is exercised by the expansion tests; the production scope \
                  surface consumes the disclosure and the roots only"
    )]
    pub(crate) fn status(&self) -> PathDependencyGraphStatus {
        self.status
    }

    /// Boundary disclosure for the scope decision. `Some` exactly when the
    /// graph state could have made the computed reach differ from the real
    /// dependency reach (`limited`: partial edge inventory; `unavailable`:
    /// no adjacency at all). A `complete` graph needs no disclosure: the
    /// walk covered the full captured edge inventory.
    pub(crate) fn scope_disclosure(&self) -> Option<String> {
        match self.status {
            PathDependencyGraphStatus::Complete => None,
            PathDependencyGraphStatus::Limited => Some(
                "ripr: path-dependency graph status is limited: reverse-dependency diff-scope \
                 expansion used a partial edge inventory and may have missed dependents"
                    .to_string(),
            ),
            PathDependencyGraphStatus::Unavailable => Some(
                "ripr: path-dependency graph status is unavailable: no local Cargo.toml \
                 manifest was found, so reverse-dependency diff-scope expansion did not run \
                 and the diff scope stays the changed packages"
                    .to_string(),
            ),
        }
    }

    /// The computed dependent package roots, consuming the expansion.
    pub(crate) fn into_dependent_package_roots(self) -> BTreeSet<String> {
        self.dependent_package_roots
    }
}

/// Compute the reverse-dependency expansion for the changed package roots.
/// Reads the local manifest inventory (`workspace_graph_provenance`) and
/// walks the reverse adjacency; no network, no Cargo invocation. A changed
/// root whose manifest is not a graph node (no manifest at that root, or no
/// connected path edge anywhere) can still contribute through the
/// declared-target index when some manifest declared a path dependency on
/// it. `unattributed_changed_files` carries the repo-relative,
/// `/`-separated changed Rust files that the layout heuristics could not
/// place in any package; they are attributed to the nearest discovered
/// manifest directory (#3616 review).
pub(crate) fn reverse_dependent_scope_expansion(
    root: &Path,
    changed_package_roots: &BTreeSet<String>,
    unattributed_changed_files: &BTreeSet<String>,
) -> PathDependencyScopeExpansion {
    let manifest_dir_prefixes = crate::analysis::seam_cache::workspace_manifest_dir_prefixes(root);
    let provenance = workspace_graph_provenance(root);
    expansion_from_provenance(
        &provenance,
        &manifest_dir_prefixes,
        changed_package_roots,
        unattributed_changed_files,
    )
}

/// Pure core behind [`reverse_dependent_scope_expansion`]: the expansion
/// computed from one provenance snapshot plus the discovered manifest
/// directory prefixes. Kept separate so the deleted-target scenario can be
/// proved against synthetic provenance without a filesystem.
pub(crate) fn expansion_from_provenance(
    provenance: &WorkspaceGraphProvenance,
    manifest_dir_prefixes: &[String],
    changed_package_roots: &BTreeSet<String>,
    unattributed_changed_files: &BTreeSet<String>,
) -> PathDependencyScopeExpansion {
    // Fast path first: the heuristic package roots keep their existing
    // meaning, and the attribution fallback below only ever adds seeds for
    // files the heuristics dropped (#3616 review).
    let mut seed_roots: BTreeSet<String> = changed_package_roots.clone();
    // Custom Cargo target paths (`[[bin]] path = "bin/tool.rs"`, `[lib]
    // path = "lib/x.rs"`) match no layout heuristic, so those changed files
    // previously never seeded a package root and their dependents never
    // expanded. Attribute each unplaced file to the nearest discovered
    // manifest directory that is an ancestor of the file (longest prefix;
    // the root manifest is the empty prefix), using the same scan that
    // names the adjacency's manifests so the identities agree.
    for file in unattributed_changed_files {
        if let Some(directory) = manifest_dir_prefixes
            .iter()
            .filter(|prefix| file.starts_with(prefix.as_str()))
            .max_by_key(|prefix| prefix.len())
        {
            seed_roots.insert(directory.clone());
        }
    }

    let adjacency = PathDependencyAdjacency::build(provenance);
    // Declared-target index over every captured edge that carries a lexical
    // identity, resolution regardless (#3616 review): when a diff deletes
    // or renames a dependency directory, the declaring manifests' edges are
    // `TargetMissing` and stay disconnected after the #3613 hardening, but
    // the declarers remain real declared dependents of the identity they
    // named — and a build error is exactly when the dependents' tests
    // belong in scope. One hop, no walk, so cycles cannot loop here.
    let mut declared_dependents: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for edge in &provenance.path_dependency_edges {
        let Some(resolved) = edge.resolved_path.as_deref() else {
            continue;
        };
        declared_dependents
            .entry(manifest_identity_from_resolved_path(resolved))
            .or_default()
            .insert(edge.from_manifest.clone());
    }

    let mut dependent_package_roots = BTreeSet::new();
    for changed in &seed_roots {
        let manifest = manifest_of_package_root(changed);
        if let Some(walk) = adjacency.reverse_walk(&manifest) {
            for reached in walk.reachable() {
                dependent_package_roots.insert(package_root_of_manifest(reached));
            }
        }
        if let Some(declarers) = declared_dependents.get(&manifest) {
            for declarer in declarers {
                dependent_package_roots.insert(package_root_of_manifest(declarer));
            }
        }
    }
    PathDependencyScopeExpansion {
        status: adjacency.status(),
        dependent_package_roots,
        manifest_dir_prefixes: manifest_dir_prefixes.to_vec(),
    }
}

/// Manifest identity of a `package_root`-shaped directory prefix: the empty
/// root is the scan-root manifest, every other root already ends in `/`.
fn manifest_of_package_root(package_root: &str) -> String {
    format!("{package_root}Cargo.toml")
}

/// `package_root`-shaped directory prefix of an adjacency manifest node.
/// A node that does not end in `Cargo.toml` cannot name a package root, so
/// the identity is kept as-is and matches no workspace file (fail closed:
/// nothing is selected through an unmappable node).
fn package_root_of_manifest(manifest: &str) -> String {
    manifest
        .strip_suffix("Cargo.toml")
        .unwrap_or(manifest)
        .to_string()
}

/// Node identity of one resolved dependency path. Cargo path dependencies
/// name either a directory (its manifest is `<dir>/Cargo.toml`) or a manifest
/// file directly; the resolved capture identity already uses `/` separators.
fn manifest_identity_from_resolved_path(resolved: &str) -> String {
    if resolved == "Cargo.toml" || resolved.ends_with("/Cargo.toml") {
        resolved.to_string()
    } else if resolved.is_empty() {
        "Cargo.toml".to_string()
    } else {
        format!("{resolved}/Cargo.toml")
    }
}

/// Status detail: name the honest boundary whenever one exists — a partial
/// edge inventory, an empty inventory, or edges that cannot participate
/// because they carry no resolved identity.
fn compose_detail(
    status: PathDependencyGraphStatus,
    edge_count: usize,
    connected_edge_count: usize,
    provenance: &WorkspaceGraphProvenance,
) -> Option<String> {
    let mut detail = match status {
        PathDependencyGraphStatus::Limited => Some(format!(
            "path-dependency edge capture reported {} limitation(s); the adjacency covers \
             a partial edge inventory",
            provenance.path_dependency_limitations.len()
        )),
        PathDependencyGraphStatus::Complete if edge_count == 0 => Some(
            "no path dependencies were declared in the scanned manifests; the adjacency \
             has no nodes"
                .to_string(),
        ),
        PathDependencyGraphStatus::Complete | PathDependencyGraphStatus::Unavailable => None,
    };
    let disconnected = edge_count - connected_edge_count;
    if disconnected > 0 {
        let boundary = format!(
            "{disconnected} of {edge_count} edges do not resolve to an existing \
             in-workspace manifest and do not participate in the adjacency"
        );
        detail = Some(match detail {
            Some(existing) => format!("{existing}; {boundary}"),
            None => boundary,
        });
    }
    detail
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::seam_cache::{
        PathDependencyEdge, PathDependencyLimitation, PathDependencyLimitationKind,
        PathDependencyResolution, PathDependencySection, PathDependencySource,
        workspace_graph_provenance,
    };
    use std::path::{Path, PathBuf};

    fn unique_dir(label: &str) -> PathBuf {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "ripr-path-dep-adjacency-{label}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn write_manifest(root: &Path, relative: &str, text: &str) -> Result<(), String> {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("create {}: {err}", parent.display()))?;
        }
        std::fs::write(&path, text).map_err(|err| format!("write {}: {err}", path.display()))
    }

    fn adjacency_for(root: &Path) -> PathDependencyAdjacency {
        PathDependencyAdjacency::build(&workspace_graph_provenance(root))
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    /// The task matrix: a -> b -> c through path dependencies plus an
    /// unrelated crate d. Fails under a plausible wrong implementation such
    /// as swapped forward/reverse maps (forward(a) would be empty and
    /// reverse(c) would lose a) or an order-dependent build.
    #[test]
    fn adjacency_walks_forward_and_reverse_over_a_path_dep_chain() -> Result<(), String> {
        let root = unique_dir("chain");
        let _ = std::fs::remove_dir_all(&root);
        for (name, deps) in [
            ("a", "b = { path = \"../b\" }"),
            ("b", "c = { path = \"../c\" }"),
            ("c", ""),
            ("d", ""),
        ] {
            let mut manifest = format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n");
            if !deps.is_empty() {
                manifest.push_str(&format!("\n[dependencies]\n{deps}\n"));
            }
            write_manifest(&root, &format!("{name}/Cargo.toml"), &manifest)?;
        }

        let adjacency = adjacency_for(&root);
        assert_eq!(
            adjacency.status(),
            PathDependencyGraphStatus::Complete,
            "a fully parsed chain has no capture limitation"
        );
        assert_eq!(adjacency.edge_count(), 2);
        assert_eq!(adjacency.connected_edge_count(), 2);
        assert_eq!(adjacency.nodes().len(), 3, "d is isolated: {adjacency:?}");
        assert!(!adjacency.contains_node("d/Cargo.toml"));

        // Direct neighbors: forward is the declared direction, reverse the
        // depended-on direction. Swapping the two maps fails here first.
        assert_eq!(
            adjacency
                .forward_neighbors("a/Cargo.toml")
                .map(|set| set.len()),
            Some(1)
        );
        assert!(
            adjacency
                .forward_neighbors("a/Cargo.toml")
                .is_some_and(|set| set.contains("b/Cargo.toml"))
        );
        assert!(
            adjacency
                .forward_neighbors("b/Cargo.toml")
                .is_some_and(|set| set.contains("c/Cargo.toml"))
        );
        assert_eq!(adjacency.forward_neighbors("c/Cargo.toml"), None);
        assert!(
            adjacency
                .reverse_neighbors("c/Cargo.toml")
                .is_some_and(|set| set.contains("b/Cargo.toml"))
        );
        assert!(
            adjacency
                .reverse_neighbors("b/Cargo.toml")
                .is_some_and(|set| set.contains("a/Cargo.toml"))
        );
        assert_eq!(adjacency.reverse_neighbors("a/Cargo.toml"), None);

        // Transitive walks.
        let forward_a = adjacency
            .forward_walk("a/Cargo.toml")
            .ok_or_else(|| "a must be a graph node".to_string())?;
        assert_eq!(
            forward_a.reachable(),
            strings(&["b/Cargo.toml", "c/Cargo.toml"])
        );
        assert!(forward_a.cycle_manifests().is_empty());
        let forward_b = adjacency
            .forward_walk("b/Cargo.toml")
            .ok_or_else(|| "b must be a graph node".to_string())?;
        assert_eq!(forward_b.reachable(), strings(&["c/Cargo.toml"]));
        let reverse_c = adjacency
            .reverse_walk("c/Cargo.toml")
            .ok_or_else(|| "c must be a graph node".to_string())?;
        assert_eq!(
            reverse_c.reachable(),
            strings(&["a/Cargo.toml", "b/Cargo.toml"])
        );
        let reverse_a = adjacency
            .reverse_walk("a/Cargo.toml")
            .ok_or_else(|| "a must be a graph node".to_string())?;
        assert!(reverse_a.reachable().is_empty(), "nothing reaches a");

        // Isolated crate d: not a node, so both walks name that explicitly
        // instead of returning an empty walk.
        assert_eq!(adjacency.forward_walk("d/Cargo.toml"), None);
        assert_eq!(adjacency.reverse_walk("d/Cargo.toml"), None);

        // Deterministic rebuild: same input, byte-equal graph.
        assert_eq!(adjacency, adjacency_for(&root));

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// Cycles terminate and are disclosed as cycle markers: a walk over a
    /// two-manifest cycle and over a crate feeding the cycle both finish.
    #[test]
    fn adjacency_cycle_walk_terminates_and_discloses_the_cycle_marker() -> Result<(), String> {
        let root = unique_dir("cycle");
        let _ = std::fs::remove_dir_all(&root);
        write_manifest(
            &root,
            "x/Cargo.toml",
            "[package]\nname = \"x\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\ny = { path = \"../y\" }\n",
        )?;
        write_manifest(
            &root,
            "y/Cargo.toml",
            "[package]\nname = \"y\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\nx = { path = \"../x\" }\n",
        )?;
        write_manifest(
            &root,
            "z/Cargo.toml",
            "[package]\nname = \"z\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\ny = { path = \"../y\" }\n",
        )?;

        let adjacency = adjacency_for(&root);
        let walk_x = adjacency
            .forward_walk("x/Cargo.toml")
            .ok_or_else(|| "x must be a graph node".to_string())?;
        assert_eq!(walk_x.reachable(), strings(&["y/Cargo.toml"]));
        assert_eq!(
            walk_x.cycle_manifests(),
            strings(&["x/Cargo.toml"]),
            "re-entering the start through the cycle is the disclosed marker"
        );

        let walk_z = adjacency
            .forward_walk("z/Cargo.toml")
            .ok_or_else(|| "z must be a graph node".to_string())?;
        assert_eq!(
            walk_z.reachable(),
            strings(&["x/Cargo.toml", "y/Cargo.toml"])
        );
        assert_eq!(
            walk_z.cycle_manifests(),
            strings(&["y/Cargo.toml"]),
            "the back-edge into the already-visited cycle member is cut and named"
        );

        let reverse_x = adjacency
            .reverse_walk("x/Cargo.toml")
            .ok_or_else(|| "x must be a graph node".to_string())?;
        assert_eq!(
            reverse_x.reachable(),
            strings(&["y/Cargo.toml", "z/Cargo.toml"])
        );

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// A manifest declaring a path dependency on itself yields one deduped
    /// self-neighbor and a disclosed self-cycle, not a non-terminating walk.
    #[test]
    fn adjacency_self_loop_deduplicates_and_discloses() -> Result<(), String> {
        let root = unique_dir("self-loop");
        let _ = std::fs::remove_dir_all(&root);
        write_manifest(
            &root,
            "s/Cargo.toml",
            "[package]\nname = \"s\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\ns = { path = \".\" }\n",
        )?;

        let adjacency = adjacency_for(&root);
        let neighbors = adjacency
            .forward_neighbors("s/Cargo.toml")
            .ok_or_else(|| "s must have forward neighbors".to_string())?;
        assert_eq!(
            neighbors.iter().collect::<Vec<_>>(),
            vec!["s/Cargo.toml"],
            "the self-loop is a single deduplicated neighbor"
        );
        let walk = adjacency
            .forward_walk("s/Cargo.toml")
            .ok_or_else(|| "s must be a graph node".to_string())?;
        assert!(walk.reachable().is_empty(), "the self-loop adds no reach");
        assert_eq!(walk.cycle_manifests(), strings(&["s/Cargo.toml"]));

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// Status honesty: no manifests means the graph was not built; manifests
    /// without any path dependency mean a complete-but-empty graph that says
    /// so; an unparsed manifest means the inventory is partial and the status
    /// degrades to `limited`.
    #[test]
    fn adjacency_status_names_each_honesty_case() -> Result<(), String> {
        let missing_root = unique_dir("status-missing");
        let _ = std::fs::remove_dir_all(&missing_root);
        let missing = adjacency_for(&missing_root);
        assert_eq!(missing.status(), PathDependencyGraphStatus::Unavailable);
        assert_eq!(
            missing.detail(),
            Some(
                "no local Cargo.toml manifest was found; the path-dependency adjacency was not built"
            )
        );
        assert_eq!(missing.nodes().len(), 0);

        let empty_root = unique_dir("status-empty");
        let _ = std::fs::remove_dir_all(&empty_root);
        write_manifest(
            &empty_root,
            "Cargo.toml",
            "[package]\nname = \"solo\"\nversion = \"0.1.0\"\n",
        )?;
        let empty = adjacency_for(&empty_root);
        assert_eq!(empty.status(), PathDependencyGraphStatus::Complete);
        assert_eq!(empty.edge_count(), 0);
        assert_eq!(empty.nodes().len(), 0);
        assert_eq!(
            empty.detail(),
            Some(
                "no path dependencies were declared in the scanned manifests; the adjacency has no nodes"
            ),
            "an empty graph must say there were no path dependencies, not imply success silently"
        );

        let limited_root = unique_dir("status-limited");
        let _ = std::fs::remove_dir_all(&limited_root);
        write_manifest(&limited_root, "Cargo.toml", "[package\nname = \"broken\"\n")?;
        write_manifest(
            &limited_root,
            "ok/Cargo.toml",
            "[package]\nname = \"ok\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\ninner = { path = \"inner\" }\n",
        )?;
        write_manifest(
            &limited_root,
            "ok/inner/Cargo.toml",
            "[package]\nname = \"inner\"\nversion = \"0.1.0\"\n",
        )?;
        let limited = adjacency_for(&limited_root);
        assert_eq!(limited.status(), PathDependencyGraphStatus::Limited);
        assert!(
            limited
                .detail()
                .is_some_and(|detail| detail.contains("partial edge inventory"))
        );
        assert!(limited.contains_node("ok/Cargo.toml"));
        assert!(limited.contains_node("ok/inner/Cargo.toml"));

        let _ = std::fs::remove_dir_all(&empty_root);
        let _ = std::fs::remove_dir_all(&limited_root);
        Ok(())
    }

    /// The same target declared in two sections of one manifest collapses to
    /// one neighbor: the adjacency is node-level, the counts stay edge-level.
    #[test]
    fn adjacency_deduplicates_multiple_declarations_of_the_same_target() -> Result<(), String> {
        let root = unique_dir("dedup");
        let _ = std::fs::remove_dir_all(&root);
        write_manifest(
            &root,
            "Cargo.toml",
            "[workspace]\nmembers = [\"app\", \"tool\"]\n",
        )?;
        write_manifest(
            &root,
            "tool/Cargo.toml",
            "[package]\nname = \"tool\"\nversion = \"0.1.0\"\n",
        )?;
        write_manifest(
            &root,
            "app/Cargo.toml",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\ntool = { path = \"../tool\" }\n\n\
             [dev-dependencies]\ntool = { path = \"../tool\" }\n",
        )?;

        let adjacency = adjacency_for(&root);
        assert_eq!(adjacency.edge_count(), 2, "both section edges are captured");
        assert_eq!(adjacency.connected_edge_count(), 2);
        let neighbors = adjacency
            .forward_neighbors("app/Cargo.toml")
            .ok_or_else(|| "app must have forward neighbors".to_string())?;
        assert_eq!(
            neighbors.iter().collect::<Vec<_>>(),
            vec!["tool/Cargo.toml"],
            "two declarations of the same target are one adjacency neighbor"
        );
        assert_eq!(
            adjacency
                .forward_dependency_declarations("app/Cargo.toml", "tool/Cargo.toml")
                .map(|declarations| declarations.len()),
            Some(2),
            "both declared sections stay on the deduplicated pair"
        );

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// #2972 review: the declaration-aware forward lookup keeps the captured
    /// Cargo section and the declared dependency name per (declarer, target)
    /// pair, deduplicated like the neighbor sets and absent for unconnected
    /// pairs — including the reverse direction, which must not read as a
    /// forward edge.
    #[test]
    fn forward_dependency_declarations_keep_section_and_declared_name() -> Result<(), String> {
        let root = unique_dir("sections");
        let _ = std::fs::remove_dir_all(&root);
        write_manifest(
            &root,
            "Cargo.toml",
            "[workspace]\nmembers = [\"app\", \"tool\", \"buildtool\"]\n",
        )?;
        write_manifest(
            &root,
            "tool/Cargo.toml",
            "[package]\nname = \"tool\"\nversion = \"0.1.0\"\n",
        )?;
        write_manifest(
            &root,
            "buildtool/Cargo.toml",
            "[package]\nname = \"buildtool\"\nversion = \"0.1.0\"\n",
        )?;
        write_manifest(
            &root,
            "app/Cargo.toml",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\ntool = { path = \"../tool\" }\n\n\
             [build-dependencies]\nbuildtool = { path = \"../buildtool\" }\n",
        )?;

        let adjacency = adjacency_for(&root);
        let tool_declarations = adjacency
            .forward_dependency_declarations("app/Cargo.toml", "tool/Cargo.toml")
            .ok_or_else(|| "the declared normal dependency must be connected".to_string())?;
        assert_eq!(
            tool_declarations.iter().collect::<Vec<_>>(),
            vec![&(PathDependencySection::Dependencies, "tool".to_string())]
        );
        let build_declarations = adjacency
            .forward_dependency_declarations("app/Cargo.toml", "buildtool/Cargo.toml")
            .ok_or_else(|| "the declared build dependency must be connected".to_string())?;
        assert_eq!(
            build_declarations.iter().collect::<Vec<_>>(),
            vec![&(
                PathDependencySection::BuildDependencies,
                "buildtool".to_string()
            )]
        );
        assert_eq!(
            adjacency.forward_dependency_declarations("tool/Cargo.toml", "app/Cargo.toml"),
            None,
            "the reverse direction is not a forward edge"
        );
        assert_eq!(
            adjacency.forward_dependency_declarations("ghost/Cargo.toml", "tool/Cargo.toml"),
            None,
            "an unknown declarer has no forward declarations"
        );

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// Edges without a resolved identity cannot participate; the remainder is
    /// disclosed through the counts and the detail instead of being dropped
    /// silently. Synthetic provenance keeps this matrix platform-free.
    #[test]
    fn adjacency_excludes_edges_without_resolved_identity_and_discloses_the_remainder()
    -> Result<(), String> {
        let edges = vec![
            edge_with_resolution(PathDependencyResolution::Resolved, Some("lib/dep")),
            edge_with_resolution(PathDependencyResolution::UnsupportedAbsolutePath, None),
            edge_with_resolution(
                PathDependencyResolution::UnresolvedWorkspaceInheritance,
                None,
            ),
            edge_with_resolution(PathDependencyResolution::InvalidDeclaration, None),
        ];
        let provenance = WorkspaceGraphProvenance {
            package_graph_status: "complete".to_string(),
            path_dependency_edges: edges,
            path_dependency_limitations: Vec::new(),
            ..WorkspaceGraphProvenance::default()
        };
        let adjacency = PathDependencyAdjacency::build(&provenance);
        assert_eq!(adjacency.status(), PathDependencyGraphStatus::Complete);
        assert_eq!(adjacency.edge_count(), 4);
        assert_eq!(adjacency.connected_edge_count(), 1);
        assert_eq!(
            adjacency.detail(),
            Some(
                "3 of 4 edges do not resolve to an existing in-workspace manifest and do \
                 not participate in the adjacency"
            )
        );
        let neighbors = adjacency
            .forward_neighbors("app/Cargo.toml")
            .ok_or_else(|| "the connected edge must participate in the adjacency".to_string())?;
        assert_eq!(
            neighbors.iter().collect::<Vec<_>>(),
            vec!["lib/dep/Cargo.toml"]
        );
        Ok(())
    }

    /// A partial inventory with disconnected edges composes both disclosures
    /// into one detail line.
    #[test]
    fn adjacency_limited_status_composes_both_disclosures() {
        let provenance = WorkspaceGraphProvenance {
            package_graph_status: "complete".to_string(),
            path_dependency_edges: vec![edge_with_resolution(
                PathDependencyResolution::UnsupportedAbsolutePath,
                None,
            )],
            path_dependency_limitations: vec![PathDependencyLimitation {
                manifest: "crates/app/Cargo.toml".to_string(),
                kind: PathDependencyLimitationKind::UnfollowedWorkspaceRedirect,
                detail: "redirect not followed".to_string(),
            }],
            ..WorkspaceGraphProvenance::default()
        };
        let adjacency = PathDependencyAdjacency::build(&provenance);
        assert_eq!(adjacency.status(), PathDependencyGraphStatus::Limited);
        let detail = adjacency.detail().unwrap_or_default();
        assert!(
            detail.contains("1 limitation(s)"),
            "the partial-inventory boundary must be named: {detail}"
        );
        assert!(
            detail.contains("1 of 1 edges do not resolve to an existing in-workspace manifest"),
            "the non-participating edge count must be named: {detail}"
        );
    }

    /// Outside-root and missing-target resolutions carry a lexical identity
    /// but do not name an existing in-workspace manifest: they must stay
    /// disconnected so receipts cannot report phantom or external manifests
    /// as adjacency nodes (#3613 review).
    #[test]
    fn adjacency_excludes_outside_root_and_missing_target_edges() -> Result<(), String> {
        let mut stray =
            edge_with_resolution(PathDependencyResolution::TargetMissing, Some("ghost"));
        stray.from_manifest = "stray/Cargo.toml".to_string();
        let edges = vec![
            edge_with_resolution(PathDependencyResolution::Resolved, Some("lib/dep")),
            edge_with_resolution(
                PathDependencyResolution::ResolvedOutsideWorkspace,
                Some("../outside"),
            ),
            stray,
        ];
        let provenance = WorkspaceGraphProvenance {
            package_graph_status: "complete".to_string(),
            path_dependency_edges: edges,
            path_dependency_limitations: Vec::new(),
            ..WorkspaceGraphProvenance::default()
        };
        let adjacency = PathDependencyAdjacency::build(&provenance);
        assert_eq!(adjacency.edge_count(), 3);
        assert_eq!(adjacency.connected_edge_count(), 1);
        assert!(
            !adjacency.contains_node("../outside/Cargo.toml"),
            "an outside-root resolution must not become a node"
        );
        assert!(
            !adjacency.contains_node("ghost/Cargo.toml"),
            "a missing-target resolution must not become a node"
        );
        assert!(
            !adjacency.contains_node("stray/Cargo.toml"),
            "a manifest whose every edge is disconnected must not appear as an \
             isolated participant (#3613 review)"
        );
        assert!(
            adjacency
                .forward_neighbors("app/Cargo.toml")
                .is_some_and(
                    |neighbors| neighbors.iter().collect::<Vec<_>>() == vec!["lib/dep/Cargo.toml"]
                ),
            "only the resolved in-workspace edge participates"
        );
        Ok(())
    }

    /// `unavailable` is reserved for an absent manifest scan: provenance that
    /// captured a partial inventory plus limitations must stay `limited` with
    /// its edges, not collapse into a bare `unavailable` that discards the
    /// capture (#3613 review).
    #[test]
    fn unavailable_provenance_with_limitations_stays_limited_and_keeps_edges() {
        let provenance = WorkspaceGraphProvenance {
            package_graph_status: "unavailable".to_string(),
            path_dependency_edges: vec![edge_with_resolution(
                PathDependencyResolution::Resolved,
                Some("lib/dep"),
            )],
            path_dependency_limitations: vec![PathDependencyLimitation {
                manifest: "crates/app/Cargo.toml".to_string(),
                kind: PathDependencyLimitationKind::UnfollowedWorkspaceRedirect,
                detail: "redirect not followed".to_string(),
            }],
            ..WorkspaceGraphProvenance::default()
        };
        let adjacency = PathDependencyAdjacency::build(&provenance);
        assert_eq!(adjacency.status(), PathDependencyGraphStatus::Limited);
        assert_eq!(adjacency.connected_edge_count(), 1);
        assert!(
            adjacency
                .detail()
                .is_some_and(|detail| detail.contains("1 limitation(s)")),
            "the capture limitations must be disclosed: {:?}",
            adjacency.detail()
        );
    }

    fn edge_with_resolution(
        resolution: PathDependencyResolution,
        resolved_path: Option<&str>,
    ) -> PathDependencyEdge {
        PathDependencyEdge {
            from_manifest: "app/Cargo.toml".to_string(),
            section: PathDependencySection::Dependencies,
            target: None,
            dependency_name: "dep".to_string(),
            declared_path: Some("../dep".to_string()),
            resolved_path: resolved_path.map(str::to_string),
            resolution,
            source: PathDependencySource::Package,
        }
    }

    /// Identity normalization: directories gain `/Cargo.toml`, manifest-file
    /// shapes are kept, and the scan root resolves to `Cargo.toml`.
    #[test]
    fn manifest_identity_from_resolved_path_normalizes_each_shape() {
        assert_eq!(manifest_identity_from_resolved_path("b"), "b/Cargo.toml");
        assert_eq!(
            manifest_identity_from_resolved_path("crates/shared"),
            "crates/shared/Cargo.toml"
        );
        assert_eq!(
            manifest_identity_from_resolved_path("shared/Cargo.toml"),
            "shared/Cargo.toml",
            "a declared path already naming a manifest is kept as-is"
        );
        assert_eq!(
            manifest_identity_from_resolved_path("Cargo.toml"),
            "Cargo.toml"
        );
        assert_eq!(
            manifest_identity_from_resolved_path(""),
            "Cargo.toml",
            "an empty resolution is the scan root manifest"
        );
    }

    /// A start node whose only role is being depended on (a reverse-side
    /// leaf) is a node: its forward walk exists and is empty, which is
    /// distinct from `None` (not a node at all).
    #[test]
    fn adjacency_reverse_leaf_has_an_empty_forward_walk() -> Result<(), String> {
        let root = unique_dir("reverse-leaf");
        let _ = std::fs::remove_dir_all(&root);
        write_manifest(
            &root,
            "Cargo.toml",
            "[workspace]\nmembers = [\"app\", \"lib\"]\n",
        )?;
        write_manifest(
            &root,
            "lib/Cargo.toml",
            "[package]\nname = \"lib\"\nversion = \"0.1.0\"\n",
        )?;
        write_manifest(
            &root,
            "app/Cargo.toml",
            "[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\nlib = { path = \"../lib\" }\n",
        )?;

        let adjacency = adjacency_for(&root);
        let walk = adjacency
            .forward_walk("lib/Cargo.toml")
            .ok_or_else(|| "lib must be a graph node".to_string())?;
        assert!(walk.reachable().is_empty());
        assert!(walk.cycle_manifests().is_empty());
        assert_eq!(adjacency.reverse_neighbors("app/Cargo.toml"), None);

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    // --- #2970 slice C: reverse-dependency diff-scope expansion ---

    fn chain_workspace(root: &Path) -> Result<(), String> {
        // a <- b <- c: b declares a, c declares b. d is unrelated.
        write_manifest(
            root,
            "Cargo.toml",
            "[workspace]\nmembers = [\"a\", \"b\", \"c\", \"d\"]\n",
        )?;
        write_manifest(
            root,
            "a/Cargo.toml",
            "[package]\nname = \"a\"\nversion = \"0.1.0\"\n",
        )?;
        write_manifest(
            root,
            "b/Cargo.toml",
            "[package]\nname = \"b\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\na = { path = \"../a\" }\n",
        )?;
        write_manifest(
            root,
            "c/Cargo.toml",
            "[package]\nname = \"c\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\nb = { path = \"../b\" }\n",
        )?;
        write_manifest(
            root,
            "d/Cargo.toml",
            "[package]\nname = \"d\"\nversion = \"0.1.0\"\n",
        )
    }

    fn roots(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    /// Expansion with no unattributed files: the heuristic fast path.
    fn expansion_for(root: &Path, changed: &BTreeSet<String>) -> PathDependencyScopeExpansion {
        reverse_dependent_scope_expansion(root, changed, &BTreeSet::new())
    }

    /// The task matrix: changing `a` reaches dependents `b` and `c` through
    /// the reverse adjacency, the unrelated `d` contributes nothing, and the
    /// direction is discriminated on both ends — expanding from `c` (which
    /// nothing depends on) yields nothing. A swapped forward/reverse build
    /// would fail both halves: `a` would reach nothing and `c` would reach
    /// `b` and `a`.
    #[test]
    fn expansion_reaches_dependents_of_changed_packages_only() -> Result<(), String> {
        let root = unique_dir("expansion-chain");
        let _ = std::fs::remove_dir_all(&root);
        chain_workspace(&root)?;

        let from_a = expansion_for(&root, &roots(&["a/"]));
        assert_eq!(from_a.status(), PathDependencyGraphStatus::Complete);
        assert_eq!(
            from_a.scope_disclosure(),
            None,
            "a complete graph needs no scope disclosure"
        );
        assert_eq!(
            from_a.into_dependent_package_roots(),
            roots(&["b/", "c/"]),
            "changing a must bring its transitive dependents b and c into scope"
        );

        let from_c = expansion_for(&root, &roots(&["c/"]));
        assert_eq!(from_c.status(), PathDependencyGraphStatus::Complete);
        assert!(
            from_c.into_dependent_package_roots().is_empty(),
            "nothing depends on c, so expanding from c must add nothing"
        );

        let from_a_and_d = expansion_for(&root, &roots(&["a/", "d/"]));
        assert_eq!(
            from_a_and_d.into_dependent_package_roots(),
            roots(&["b/", "c/"]),
            "the unrelated crate d must stay out of the expansion"
        );

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// Changed roots with no manifest node (an unknown package root, a root
    /// whose crate has no path edges) expand to nothing without inventing
    /// scope; an empty changed set is an empty expansion.
    #[test]
    fn expansion_adds_nothing_for_unmapped_or_empty_changed_roots() -> Result<(), String> {
        let root = unique_dir("expansion-unmapped");
        let _ = std::fs::remove_dir_all(&root);
        chain_workspace(&root)?;

        let unknown = expansion_for(&root, &roots(&["crates/ghost/"]));
        assert_eq!(unknown.status(), PathDependencyGraphStatus::Complete);
        assert!(unknown.into_dependent_package_roots().is_empty());

        let isolated = expansion_for(&root, &roots(&["d/"]));
        assert!(
            isolated.into_dependent_package_roots().is_empty(),
            "d participates in no path edge, so it has no dependents to add"
        );

        let empty = expansion_for(&root, &BTreeSet::new());
        assert_eq!(empty.status(), PathDependencyGraphStatus::Complete);
        assert!(empty.into_dependent_package_roots().is_empty());

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// Honesty: an absent manifest scan must disclose that the expansion did
    /// not run, and a partial edge inventory must disclose that dependents
    /// may be missing. Neither state may read as a complete reach.
    #[test]
    fn expansion_discloses_limited_and_unavailable_graph_states() -> Result<(), String> {
        let missing_root = unique_dir("expansion-missing");
        let _ = std::fs::remove_dir_all(&missing_root);
        let missing = expansion_for(&missing_root, &roots(&["a/"]));
        assert_eq!(missing.status(), PathDependencyGraphStatus::Unavailable);
        let unavailable_disclosure = missing
            .scope_disclosure()
            .ok_or_else(|| "an unavailable graph must disclose the scope boundary".to_string())?;
        assert!(
            missing.into_dependent_package_roots().is_empty(),
            "an unavailable graph must not fabricate reach"
        );
        for needle in [
            "unavailable",
            "expansion did not run",
            "stays the changed packages",
        ] {
            assert!(
                unavailable_disclosure.contains(needle),
                "disclosure must name `{needle}`: {unavailable_disclosure}"
            );
        }

        let limited_root = unique_dir("expansion-limited");
        let _ = std::fs::remove_dir_all(&limited_root);
        write_manifest(&limited_root, "Cargo.toml", "[package\nname = \"broken\"\n")?;
        chain_dependents_under_broken_root(&limited_root)?;
        let limited = expansion_for(&limited_root, &roots(&["a/"]));
        assert_eq!(limited.status(), PathDependencyGraphStatus::Limited);
        let limited_disclosure = limited
            .scope_disclosure()
            .ok_or_else(|| "a limited graph must disclose the partial inventory".to_string())?;
        assert_eq!(
            limited.into_dependent_package_roots(),
            roots(&["b/"]),
            "the connected edge still participates under a limited inventory"
        );
        assert!(
            limited_disclosure.contains("limited") && limited_disclosure.contains("partial"),
            "disclosure must name the limited partial inventory: {limited_disclosure}"
        );

        let _ = std::fs::remove_dir_all(&missing_root);
        let _ = std::fs::remove_dir_all(&limited_root);
        Ok(())
    }

    /// Under a root whose own manifest is unparsed, the a -> b edge between
    /// member manifests still resolves: b depends on a.
    fn chain_dependents_under_broken_root(root: &Path) -> Result<(), String> {
        write_manifest(
            root,
            "a/Cargo.toml",
            "[package]\nname = \"a\"\nversion = \"0.1.0\"\n",
        )?;
        write_manifest(
            root,
            "b/Cargo.toml",
            "[package]\nname = \"b\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\na = { path = \"../a\" }\n",
        )
    }

    /// Identity mapping between `package_root` shapes and adjacency manifest
    /// nodes round-trips, and an unmappable node fails closed to a root that
    /// matches nothing.
    #[test]
    fn package_root_and_manifest_identities_round_trip() {
        assert_eq!(manifest_of_package_root(""), "Cargo.toml");
        assert_eq!(manifest_of_package_root("crates/b/"), "crates/b/Cargo.toml");
        assert_eq!(package_root_of_manifest("Cargo.toml"), "");
        assert_eq!(package_root_of_manifest("crates/b/Cargo.toml"), "crates/b/");
        assert_eq!(
            package_root_of_manifest("not-a-manifest"),
            "not-a-manifest",
            "an unmappable node keeps its identity and matches no package root"
        );
    }

    /// #3616 review fix 1: a crate whose package uses custom Cargo target
    /// paths (`[lib] path = "lib/core.rs"`, `[[bin]] path = "bin/tool.rs"`)
    /// matches no layout heuristic, so its changed files carry no package
    /// root. The manifest-inventory attribution must place them with the
    /// nearest discovered manifest directory (longest prefix) so their
    /// dependents still expand, while the heuristic fast path stays
    /// untouched.
    #[test]
    fn expansion_attributes_custom_target_files_via_the_manifest_inventory() -> Result<(), String> {
        let root = unique_dir("expansion-custom-target");
        let _ = std::fs::remove_dir_all(&root);
        write_manifest(
            &root,
            "Cargo.toml",
            "[workspace]\nmembers = [\"t\", \"u\", \"t/inner\"]\n",
        )?;
        write_manifest(
            &root,
            "t/Cargo.toml",
            "[package]\nname = \"t\"\nversion = \"0.1.0\"\n\n\
             [lib]\npath = \"lib/core.rs\"\n\n\
             [[bin]]\nname = \"t-tool\"\npath = \"bin/tool.rs\"\n",
        )?;
        write_manifest(
            &root,
            "t/inner/Cargo.toml",
            "[package]\nname = \"t_inner\"\nversion = \"0.1.0\"\n",
        )?;
        write_manifest(
            &root,
            "u/Cargo.toml",
            "[package]\nname = \"u\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\nt = { path = \"../t\" }\n",
        )?;

        // Custom-target files of t: no heuristic root, attributed to "t/"
        // by longest prefix, whose dependent u expands.
        let custom = reverse_dependent_scope_expansion(
            &root,
            &BTreeSet::new(),
            &roots(&["t/lib/core.rs", "t/bin/tool.rs"]),
        );
        assert_eq!(custom.status(), PathDependencyGraphStatus::Complete);
        assert_eq!(custom.scope_disclosure(), None);
        assert_eq!(
            custom.into_dependent_package_roots(),
            roots(&["u/"]),
            "the custom-target crate's dependents must expand"
        );

        // Longest-prefix discrimination: a custom target inside the nested
        // t/inner package attributes to "t/inner/", whose dependents are
        // none — not to "t/", which would have pulled u in.
        let nested = reverse_dependent_scope_expansion(
            &root,
            &BTreeSet::new(),
            &roots(&["t/inner/lib/core.rs"]),
        );
        assert!(
            nested.into_dependent_package_roots().is_empty(),
            "the nested package owns the file; t's dependent must not expand"
        );

        // A file under no manifest directory other than the scan root
        // attributes to the root manifest prefix.
        let root_level =
            reverse_dependent_scope_expansion(&root, &BTreeSet::new(), &roots(&["bin/root.rs"]));
        assert!(
            root_level.into_dependent_package_roots().is_empty(),
            "the root manifest is not a graph node, so attribution adds a seed but no reach"
        );

        // The heuristic fast path is unchanged on the same workspace.
        let heuristic = expansion_for(&root, &roots(&["t/"]));
        assert_eq!(
            heuristic.into_dependent_package_roots(),
            roots(&["u/"]),
            "heuristic seeds must keep their existing behavior"
        );

        let _ = std::fs::remove_dir_all(&root);
        Ok(())
    }

    /// #3616 review fix 2: when a diff deletes or renames a dependency
    /// directory, the declaring manifest's edge is `TargetMissing` and stays
    /// disconnected, but the declarer is still a real declared dependent of
    /// the identity it named. The declared-target index (every captured edge
    /// with a lexical identity, resolution regardless) must reach it. One
    /// hop only: no walk runs through a broken edge.
    #[test]
    fn expansion_reaches_declared_dependents_when_the_dependency_target_is_missing() {
        let edges = vec![
            edge_from_manifest(
                "b/Cargo.toml",
                PathDependencyResolution::TargetMissing,
                Some("a"),
            ),
            edge_from_manifest(
                "c/Cargo.toml",
                PathDependencyResolution::Resolved,
                Some("b"),
            ),
            edge_from_manifest(
                "s/Cargo.toml",
                PathDependencyResolution::ResolvedOutsideWorkspace,
                Some("../outside"),
            ),
        ];
        let provenance = WorkspaceGraphProvenance {
            package_graph_status: "complete".to_string(),
            path_dependency_edges: edges,
            path_dependency_limitations: Vec::new(),
            ..WorkspaceGraphProvenance::default()
        };
        let inventory = vec!["a/".to_string(), "b/".to_string(), "c/".to_string()];

        // The connected adjacency has no a -> dependent edge (TargetMissing
        // is disconnected), so only the declared-target index reaches b.
        let from_deleted =
            expansion_from_provenance(&provenance, &inventory, &roots(&["a/"]), &BTreeSet::new());
        assert_eq!(from_deleted.status(), PathDependencyGraphStatus::Complete);
        assert_eq!(from_deleted.scope_disclosure(), None);
        assert_eq!(
            from_deleted.into_dependent_package_roots(),
            roots(&["b/"]),
            "the declarer of the missing target is a real declared dependent"
        );

        // A connected changed package unions its walk reach with its
        // declared dependents without duplication.
        let from_b =
            expansion_from_provenance(&provenance, &inventory, &roots(&["b/"]), &BTreeSet::new());
        assert_eq!(
            from_b.into_dependent_package_roots(),
            roots(&["c/"]),
            "the walk and the declared index agree on c"
        );

        // An outside-root lexical identity can never match an in-workspace
        // changed manifest, so the outside edge stays out.
        let from_outside = expansion_from_provenance(
            &provenance,
            &inventory,
            &roots(&["outside/"]),
            &BTreeSet::new(),
        );
        assert!(
            from_outside.into_dependent_package_roots().is_empty(),
            "an outside-workspace declared identity must not match an in-root seed"
        );

        // The declared index matches by identity alone: a seed whose
        // manifest is absent from the inventory still reaches its
        // declarers, but nothing declares `ghost` here.
        let from_ghost = expansion_from_provenance(
            &provenance,
            &inventory,
            &roots(&["ghost/"]),
            &BTreeSet::new(),
        );
        assert!(
            from_ghost.into_dependent_package_roots().is_empty(),
            "no edge declared ghost, so nothing expands"
        );
    }

    fn edge_from_manifest(
        from_manifest: &str,
        resolution: PathDependencyResolution,
        resolved_path: Option<&str>,
    ) -> PathDependencyEdge {
        PathDependencyEdge {
            from_manifest: from_manifest.to_string(),
            section: PathDependencySection::Dependencies,
            target: None,
            dependency_name: "dep".to_string(),
            declared_path: Some("../dep".to_string()),
            resolved_path: resolved_path.map(str::to_string),
            resolution,
            source: PathDependencySource::Package,
        }
    }
}
