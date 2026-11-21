# RIPR-SPEC-0171: path-dependency diff-scope expansion

Status: proposed

Owner:

Created: 2026-08-31

Linked proposal:

Linked ADRs:

Linked plan:

Linked issues:

- #2665 (path dependencies not followed), slice C of the program
- #2970 (expand diff scope via reverse-dependency graph)
- #2968/#3037 (slice A: captured path-dependency edges)
- #2969 (slice B: forward/reverse adjacency)

Linked PRs:

Support-tier impact:

- No tier promotion. Diff-scope expansion widens which files Draft/Fast
  index; it adds no runtime evidence and does not change any support
  tier. [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md) remains the
  tier authority.

Policy impact:

## Problem

In the modes whose diff selection narrows to changed packages (Draft/Fast
with unchanged tests included), a diff that touches only crate `a` indexes
`a`'s files. A behavior change in a path dependency can surface in every
crate that depends on it, but the tests of those dependents are dropped from
the index entirely, so no cross-crate evidence can even be considered. The
forward/reverse path-dependency adjacency built in #2969 was disclosure-only;
no analysis surface consumed it.

## Behavior

In the package-narrowing selections (Draft/Fast with
`include_unchanged_tests`), the Rust adapter expands the changed-package
scope with the package roots of the manifests that reach a changed package
through the reverse path-dependency adjacency:

- the base scope is never dropped: expansion only ever adds packages;
- the walk is transitive (a change in `a` reaches `b` when `b` depends on
  `a` and `c` when `c` depends on `b`) and deterministic (`BTreeMap`/
  `BTreeSet`-ordered);
- cycles are cut by the walk and disclosed on the graph surfaces; the
  reachable set itself stays exact for the walked edges;
- selections without package narrowing are unchanged: Instant stays
  changed-files-only, Deep/Ready stay whole-workspace, and
  `include_unchanged_tests = false` stays changed-files-only in every mode;
- a `limited` graph (partial edge inventory) still walks its connected edges
  and discloses that dependents may be missing; an `unavailable` graph (no
  manifest scan) discloses that the expansion did not run and the scope
  stays the changed packages. Neither state may read as a complete reach;
- a `complete` graph needs no disclosure: the walk covered the full captured
  edge inventory, and a complete-but-empty graph truthfully adds nothing;
- the diff-index scope cap keeps its fail-closed behavior unchanged, and the
  expansion counts toward it: a diff whose dependent reach pushes the index
  over the cap fails closed with `diff_scope_oversized` exactly as before;
- changed files that match no layout heuristic — custom Cargo target paths
  such as `[lib] path = "lib/core.rs"` or `[[bin]] path = "bin/tool.rs"`
  (#3616 review) — are attributed to the nearest discovered manifest
  directory that is an ancestor of the file (longest prefix, the root
  manifest being the empty prefix), using the same scan that names the
  adjacency's manifests. The heuristic fast path is untouched: attribution
  only adds seeds for files the heuristics dropped, and package narrowing
  stays alive when the attributed dependents are the only narrowing input;
- a secondary declared-target index covers every captured edge that carries
  a lexical identity regardless of resolution (#3616 review): when a diff
  deletes or renames a dependency directory, the declaring manifest's edge
  is `TargetMissing` and stays disconnected, but the declarer is still a
  real declared dependent of the identity it named — exactly when a build
  error makes the dependents' tests belong in scope. This reach is one hop
  (no walk runs through a broken edge), matches by manifest identity alone
  (the declared manifest need not exist), is unioned with the connected
  walk without duplication, and is disclosed as ordinary expansion results
  with no extra disclosure.

The manifest inventory is read locally; no Cargo invocation, no registry
metadata, and no network access is added.

## Non-Goals

No forward-direction scope expansion (a changed crate's own dependencies do
not enter scope through this behavior). No classification change: the
cross-crate package-prefix guard, its uniqueness bypass, and the
`workspace_index_complete` derivation keep their semantics — expansion
participates in that derivation only through the selection it actually
produced. No cross-crate test-oracle crediting change (#2664 owns that). No
change to `input_changed` naming, parity decisions, or any report/JSON
schema. No registry or external dependency resolution.

## Required Evidence

- a disclosure line naming the `limited` boundary (partial edge inventory,
  dependents may be missing) or the `unavailable` boundary (expansion did
  not run, scope stays the changed packages) whenever the scope decision
  consults a graph in one of those states;
- end-to-end evidence that a Draft-mode diff touching only `a` surfaces the
  dependent crate's test as related evidence through the dependency edge,
  and that the same workspace without the edge does not;
- end-to-end evidence that a custom-target changed file (`[lib] path`
  outside the default layouts) expands its crate's path dependents through
  the manifest-inventory attribution;
- synthetic-provenance evidence that `TargetMissing` edges reach their
  declared dependents, that the connected walk and the declared index agree
  without duplication, and that outside-root declared identities stay out;
- selection-level evidence that the unrelated crate stays out, that
  non-narrowing selections ignore the expansion, and that attributed
  dependents keep package narrowing alive without heuristic changed roots.

## Acceptance Examples

- Workspace `a <- b <- c` (path dependencies) plus unrelated `d`: a Draft
  diff touching only `a/src/lib.rs` brings `b`'s and `c`'s package files
  into scope; `d` stays out; a `Draft` finding for `a`'s changed owner may
  name `b`'s integration test as related evidence. Expanding from `c`
  (which nothing depends on) adds nothing.
- The same workspace without `b`'s dependency edge: `b` stays out of scope.
- A workspace whose manifest inventory cannot be parsed fully: the graph is
  `limited`, connected edges still participate, and the disclosure names the
  partial inventory.
- A directory with no `Cargo.toml` at all: the graph is `unavailable`, the
  expansion did not run, and the disclosure says the scope stays the changed
  packages.
- Crate `t` declares `[lib] path = "lib/core.rs"` and crate `u` path-depends
  on `t`: a Draft diff touching only `t/lib/core.rs` (no heuristic package
  root) attributes the file to `t`, brings `u`'s package files into scope,
  and may name `u`'s integration test as related evidence; a custom target
  inside a nested package attributes to that nested package, not `t`.
- A diff deletes dependency directory `a` while `b` still declares
  `a = { path = "../a" }`: the edge is `TargetMissing` and disconnected,
  and the declared-target index still brings `b`'s package files into
  scope.

## Test Mapping

- `crates/ripr/src/analysis/workspace/path_dependencies.rs::tests::expansion_reaches_dependents_of_changed_packages_only`
- `crates/ripr/src/analysis/workspace/path_dependencies.rs::tests::expansion_adds_nothing_for_unmapped_or_empty_changed_roots`
- `crates/ripr/src/analysis/workspace/path_dependencies.rs::tests::expansion_discloses_limited_and_unavailable_graph_states`
- `crates/ripr/src/analysis/workspace/path_dependencies.rs::tests::expansion_attributes_custom_target_files_via_the_manifest_inventory`
- `crates/ripr/src/analysis/workspace/path_dependencies.rs::tests::expansion_reaches_declared_dependents_when_the_dependency_target_is_missing`
- `crates/ripr/src/analysis/workspace/path_dependencies.rs::tests::package_root_and_manifest_identities_round_trip`
- `crates/ripr/src/analysis/workspace/select.rs::tests::dependent_packages_narrow_selection_without_heuristic_changed_roots`
- `crates/ripr/src/analysis/workspace/select.rs::tests::dependent_packages_enter_draft_and_fast_selection`
- `crates/ripr/src/analysis/workspace/select.rs::tests::empty_dependent_roots_reproduce_the_unchanged_selection`
- `crates/ripr/src/analysis/workspace/select.rs::tests::dependent_roots_do_not_change_non_narrowing_selections`
- `crates/ripr/src/analysis/language/rust.rs::tests::draft_diff_scope_reaches_path_dependent_tests_through_the_dependency_edge`
- `crates/ripr/src/analysis/language/rust.rs::tests::draft_diff_scope_stays_narrow_without_the_path_dependency_edge`
- `crates/ripr/src/analysis/language/rust.rs::tests::instant_mode_does_not_expand_scope_through_path_dependencies`
- `crates/ripr/src/analysis/language/rust.rs::tests::draft_diff_scope_expands_custom_target_files_to_their_path_dependents`

## Implementation Mapping

- `crates/ripr/src/analysis/seam_cache.rs` —
  `workspace_manifest_dir_prefixes` (path-only manifest inventory sharing
  the provenance scan's skip list)
- `crates/ripr/src/analysis/workspace/path_dependencies.rs` —
  `reverse_dependent_scope_expansion`, `expansion_from_provenance`,
  `PathDependencyScopeExpansion`, package-root/manifest identity mapping
- `crates/ripr/src/analysis/workspace/select.rs` —
  `select_rust_files_for_mode_with_dependent_packages`
- `crates/ripr/src/analysis/language/rust.rs` — Draft/Fast gating,
  unattributed-file collection, disclosure emission, selection wiring

## Metrics

- unit_test_pass_rate
- golden_fixture_pass_rate
