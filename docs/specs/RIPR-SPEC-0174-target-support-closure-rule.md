# RIPR-SPEC-0174: target support closure rule for Rust scope selection

Status: proposed

Owner:

Created: 2026-09-10

Linked proposal:

Linked ADRs:

Linked plan:

Linked issues:

- #3705 (research: define sound Cargo target support closure for diff
  indexing — planning acceptance is this documented closure rule plus its
  falsifying fixture matrix, not completed analyzer behavior)
- #1985 (partial-state and cap semantics — preserved)
- #2970 and #3616 (reverse path-dependent scope — preserved)
- #3213 (evidence roles — preserved)

Linked PRs:

Support-tier impact:

- No tier change. This rule binds future narrowing work only; current
  selection behavior is unchanged.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No gate, output, or release behavior changes.

## Problem

Draft/Fast with unchanged tests selects changed packages plus reverse
path-dependent packages. In a package with separate library, binary, and
integration-test targets, a small binary-only change therefore requires a
large package-level index. A binary label alone is insufficient evidence to
omit integration or subprocess tests, so any future target-aware narrowing
must prove — per support edge — that no relevant discriminator is dropped.

## Behavior

Any target-aware narrowing of Rust scope selection MUST satisfy every
closure clause below. Until a narrowing demonstrates all of them, the
existing broad package selection and honest refusal stand.

1. Single ownership authority: default and explicit `[lib]`, `[[bin]]`,
   examples, test, bench, and custom target paths, shared modules, nested
   packages, and overlapping manifest prefixes resolve through one
   ownership authority (today: `declared_targets_from_manifest` and
   `declared_crate_root_paths_from_manifest` in
   `analysis/workspace/cargo_targets.rs` feeding the longest-prefix rule in
   `analysis/workspace/select.rs`). No second target table may silently
   re-attribute files.
2. Retained support edges: registered integration tests, `CARGO_BIN_EXE_*`
   subprocess callers, module-parent closure, `#[path]` redirections,
   `include!` fragments, and applicable `cfg` relationships stay in scope
   for a changed target unless the closure evidence supports the exclusion.
3. Unknown relationships fail closed: an unrecognized target shape, an
   unresolvable custom path, or unavailable/limited Cargo metadata
   preserves the broader scope or discloses partial/refused analysis. A
   binary label alone never omits integration or subprocess tests.
4. Reverse dependents preserved: library and shared-module changes keep
   the reverse-dependent tests of dependent packages (the #2970/#3616
   expansion). Narrowing must not trade dependent coverage for a smaller
   index.
5. Sibling exclusion needs evidence: unrelated sibling targets leave scope
   only when the closure evidence supports that exclusion, and nested
   crates inside a dependent stay out unless the nested manifest is
   itself a dependent.
6. Completeness language binds to graph coverage: any claim that a
   narrowing is complete names the support edges traversed; partial
   coverage is disclosed, never presented as complete.

## Non-Goals

Increasing or removing the file cap, switching modes to claim equivalent
proof, arbitrary directory omissions, suppressing consumer findings,
changing public output or source-role contracts, source-role precedence
changes, and a new semantic engine are all out of scope. This spec does
not implement narrowing; it states the rule narrowing must satisfy.

Requirement-level v2 blocks and PR-local implementation slices belong in
their respective authorities; do not duplicate their normative prose here.
Maintenance-review metadata is optional and non-normative. Acceptance of
this document does not imply implementation, evidence, or support.

## Required Evidence

A narrowing candidate must present, per changed target shape: the selected
file identities, the omitted identities with the supporting closure edge
for each omission, and the retained unknown-relationship disclosures. The
falsifying matrix in Test Mapping must stay green: each row fails if its
support edge is removed.

Coverage boundary: the matrix pins path-selection edges only (package
membership, dependent expansion, module-parent closure, custom-path
prefix attribution, changed-file inclusion). `CARGO_BIN_EXE_*` subprocess
callers, `#[path]` redirections, `include!` fragments, and `cfg`-derived
relationships have no falsifying row yet — the selection tests consume
path lists, not Cargo invocations, source declarations, or
conditional-compilation inputs. A narrowing candidate that excludes on
any of those shapes must add the falsifying row first; until then those
shapes keep exactly current behavior — broad where package-attributed,
fail-closed invisible where the current closure does not follow them
(e.g. external `#[path]` redirections stay invisible in narrowed modes,
#3533) — and any exclusion beyond current behavior is unsupported.

## Inputs

- The changed-file set, the workspace file list, dependent package roots,
  and manifest directory prefixes consumed by
  `select_rust_files_for_mode_with_dependent_packages`.

## Outputs

- The narrowed selection consumed by indexing; no public output changes.

## Acceptance Examples

A workspace-root package holds `src/lib.rs`, `src/main.rs`,
`tests/it.rs`, `examples/ex.rs`, and `benches/b.rs`. A binary-only change
to `src/main.rs` keeps all five under Draft/Fast today (package-together
baseline). A future narrowing may omit `tests/it.rs` only by citing the
closure edge that proves no discriminator is lost — the binary label
alone is not such an edge, so the omission stays unsupported until that
evidence exists.

A custom-path target file with no heuristic package root and no
attributed dependents selects changed files only (plus existing module
parents). That fallback is the honest current limitation, not a license
to drop the package once dependents are attributed.

## Test Mapping

- `crates/ripr/src/analysis/workspace/select.rs::tests::draft_and_fast_keep_workspace_root_multi_target_package_together`
  pins the package-together baseline; removing package narrowing loses
  `tests/it.rs`.
- `crates/ripr/src/analysis/workspace/select.rs::tests::draft_and_fast_keep_nested_multi_target_crate_together`
  pins per-crate separation for nested multi-target crates.
- `crates/ripr/src/analysis/workspace/select.rs::tests::instant_selection_leaves_integration_tests_behind_for_bin_only_change`
  is the load-bearing control: the changed-files-plus-parents path omits
  `tests/it.rs`, proving the package edge carries it.
- `crates/ripr/src/analysis/workspace/select.rs::tests::module_closure_brings_lib_sibling_for_bin_only_change`
  pins the module-parent edge for the binary shape directly.
- `crates/ripr/src/analysis/workspace/select.rs::tests::custom_target_change_without_dependents_stays_changed_files_only`
  pins the honest custom-path fallback and its omission.
- `crates/ripr/src/analysis/workspace/select.rs::tests::dependent_custom_target_files_enter_selection_by_root_prefix`
  and `nested_crate_inside_dependent_stays_out_unless_itself_dependent`
  (existing) pin the dependent-prefix edge and the nested-crate
  non-leak clause.
- `crates/ripr/src/analysis/workspace/select.rs::tests::dependent_packages_enter_draft_and_fast_selection`
  (existing) pins the reverse-dependent clause with direction
  discrimination.

## Implementation Mapping

- `crates/ripr/src/analysis/workspace/select.rs`:
  `select_rust_files_for_mode_with_dependent_packages` owns selection and
  the module-parent closure.
- `crates/ripr/src/analysis/workspace/cargo_targets.rs`:
  `declared_targets_from_manifest`,
  `declared_crate_root_paths_from_manifest`, and
  `declared_test_targets_with_harness_from_manifest` own declared
  target/test roles (roles alone do not establish the support graph).
- `crates/ripr/src/analysis/workspace/path_dependencies.rs` owns reverse
  expansion; `source_role.rs` owns role precedence (unchanged by this
  rule).

## CI Proof

Which commands and CI lanes prove it?

- `cargo test -p ripr --lib analysis::workspace::select`
- `cargo xtask fixtures`, `cargo xtask goldens check`, `cargo xtask dogfood`
- `cargo xtask check-pr` (includes spec format/numbering, traceability,
  fixture contracts)

## Metrics

What measurements show the behavior is working? Promotion decisions belong to
the applicable support and release authorities.

- The falsifying matrix stays green with zero drift; any future narrowing
  PR cites this spec and extends the matrix rather than weakening it.

## Failure Modes

- A narrowing that drops integration tests on a binary-label argument
  fails the package-together pinning tests.
- A second target table that re-attributes files without the ownership
  authority fails the nested-crate and custom-path tests.
- An unknown relationship presented as complete violates clause 6 and has
  no supporting matrix row.
