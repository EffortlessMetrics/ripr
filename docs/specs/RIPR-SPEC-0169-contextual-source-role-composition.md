# RIPR-SPEC-0169: Contextual source-role composition

Status: proposed

Owner:

Created: 2026-08-29

Linked proposal:

Linked ADRs:

Linked plan:

Linked issues: #3533 (builds on #3530 / RIPR-SPEC-0153 and #3531), #3706 (source-directory resolution controls)

Linked PRs: #3592

Support-tier impact: none

Policy impact: none

## Problem

Per-file parsing classified every function from its own file's syntax only,
while Rust compilation crosses file boundaries. An out-of-line
`#[cfg(test)] mod tests;` makes every function in the child file test-only,
and a file-level `include!` pastes a fragment into the including file's
context — but ripr's own out-of-line test-module helpers classified as
`Production` and re-entered the production seam inventory. No stop reason was
visible when a cross-file context could not be resolved, so a silently
uncomposed file was indistinguishable from a genuinely standalone one.

## Behavior

After the style normalizer (which recomputes roles from same-file text and
would stomp any earlier composition), one composition pass composes roles per
file occurrence under a closed rule set:

- **Evidence roles union; production requires both sides production.** A
  function already carrying an evidence role keeps it. A `Production`
  function inside a context that structurally requires a test build is
  granted the evidence-only `CfgTestModule` role, in both the per-file facts
  and the flat function list. Existing evidence roles are never demoted.
- **Composition only mints `CfgTestModule`** (evidence-only). It never
  creates or removes executable `TestFact`s — that stays behind the
  #3499/#3532 executable-test authority.
- **Edges.** The pass follows two edge kinds: out-of-line module
  declarations (`mod name;`, with exact string-literal `#[path]` targets
  resolved relative to the physically declaring file, and default targets
  resolved through the declaring file's module directory) and repository-local
  file-level `include!` invocations. The cfg-test requirement of a module
  declaration or an `include!` invocation itself composes with the parent
  context: a `#[cfg(test)]` gate makes the child's content test-only
  regardless of the parent's own context.
- **Crate-root identity.** Default module resolution anchors at the
  containing directory of every crate root — `mod.rs`/`lib.rs`/`main.rs`,
  layout-autodiscovered target files (`src/lib.rs`, `src/main.rs`, one file
  directly under `src/bin/`, `tests/`, `benches/`, or `examples/`), and
  manifest-declared `path = ...` targets — and at the stem directory of every
  ordinary module file. Manifest walks memoize the resolved nearest-ancestor
  answer per directory, so sibling files in one directory share the verdict.
- **Physical source-directory anchors.** A file selected by literal `#[path]`
  and an included fragment resolve default child modules from their containing
  directory, including when their filename is custom. The including unit
  remains an identity and role parent; it does not replace the fragment's
  physical file-search directory. Compiler controls with both the correct
  child and a misleading sibling pin this distinction.
- **Include conflicts preserve physical anchors.** After complete include
  enumeration, resolved targets remain physical search anchors even when their
  contextual parent requirements conflict. This does not resolve the role conflict or grant the
  fragment a role. A missing physical child cannot fall back to the fragment's
  stem directory. Physical targets are regenerated with the index, not persisted
  as an additional serialized fact contract.
- **Mixed directory anchors.** One physical file may occur both as an ordinary
  module and through a literal path, include, or crate-root context. Retain
  both physical search directories and propagate them through default child
  declarations. The single-identity index marks children from distinct
  anchors `rust_module_ambiguous_parent`, even if only one layout is indexed;
  it does not choose an anchor or mint an evidence role from partial context.
- **Unknown fails closed.** Ambiguous module ownership (two claiming
  parents, or both default layouts present), cyclic or depth-bounded chains,
  conflicting module/include contexts, dynamic or conditionally introduced
  (`cfg_attr`) `#[path]` targets, unlexable `cfg_attr` heads, and unresolved
  targets grant nothing. The earliest unresolved edge is named on the file's
  `SourceRoleProvenance` (`rust_module_ambiguous_parent`,
  `rust_module_cycle_or_depth_limit`, `rust_module_context_conflict`), and
  include-side conflicts are named on the include limitations
  (`rust_include_conflicting_cfg_requirement`). Provenance is composer-owned
  and recomputed on every index build (`serde(skip)`: composed state stays out
  of the on-disk file-fact cache, which stores pre-composition parse facts
  only; the cache schema bumps so composition-blind entries cannot serve).
- **Dual contextual ownership** (one physical file both declared as a module
  and included as a fragment) composes only when both contexts agree;
  disagreement fails closed. The two-occurrence identity the issue prefers is
  not built here; the composed single-identity result stays conservative in
  the meantime.
- **Disclosure.** Module-side stop reasons are disclosed alongside the
  existing include limitations: files whose provenance names an unresolved
  composed chain, and files with typed-unknown module targets, are listed
  with their reason codes. Absence of a composed grant stays silent — a
  missing parent in a narrowed scan is ordinary incompleteness, not a stop
  reason.

## Non-Goals

- Proving execution or replacing real mutation testing: composed roles are
  draft-mode evidence classification only.
- Building the two-occurrence identity for dual contextual ownership.
- Composing roles across inline `mod name { ... }` nesting (same-file
  membership walk already owns those members) or resolving out-of-line
  declarations nested inside inline modules (no producer).
- Cargo feature/target activation analysis; conditions are classified
  structurally, never evaluated.
- Surfacing composed roles as new JSON output fields on this issue.

## Required Evidence

- Unit tests pin: the out-of-line `#[cfg(test)]` grant and its provenance
  chain, production controls, transitive chains, exact/conditional/dynamic
  `#[path]` handling, ambiguous parents, context conflicts, the warm per-file
  cache, cfg-gated and conflicting `include!` invocations, fragment-relative
  `#[path]` anchors, crate-root identity (custom roots, autodiscovered
  targets, sibling memoization), cycle and non-ASCII controls, and the
  Unicode `cfg_attr` fail-closed regression.
- Directory-anchor controls pin both sides: a `#[path]`-selected custom
  filename and an included fragment grant the correct child's contextual role
  while leaving the misleading stem-directory or outer-unit sibling unchanged.
  Mixed ordinary/path and ordinary/include occurrences keep both children
  ambiguous, including incomplete layouts and transitive default children.
- `cargo xtask goldens check` stays green: composed roles change no pinned
  output contract.

## Inputs

- A parsed, normalized Rust index with module declarations and include
  edges, plus the workspace root for manifest-anchored crate-root identity.

## Outputs

- Per-file `SourceRoleProvenance` (edge chain plus earliest unresolved
  reason), evidence-only `CfgTestModule` grants on composed test-only
  contexts, include-side limitation entries, and one stderr disclosure line
  when module composition fails closed.

## Acceptance Examples

- `src/lib.rs` with `#[cfg(test)] mod tests;` and an out-of-line
  `src/tests.rs`: the child's unattributed helpers gain `CfgTestModule` and
  the provenance chain records the module edge.
- `#[cfg(test)] include!("fragment.rs");` beside a production unit: the
  fragment's helpers gain `CfgTestModule` through the include edge.
- `#[path = "nested/renamed.rs"] mod nested;` with `mod child;` inside
  `renamed.rs`: the child is `nested/child.rs`, not
  `nested/renamed/child.rs`. Likewise, `include!("fragments/body.rs")` with
  `mod child;` inside the fragment selects `fragments/child.rs`, not the
  outer including unit's `child.rs`.
- `#[cfg_attr(é, path = "alternate.rs")] mod imp;` with `src/imp.rs` indexed:
  the unreadable condition fails closed to a typed unknown, so the
  default-layout file is not resolved as the module child.
- Two sibling files under `tests/` with the manifest at the root: both are
  crate roots regardless of resolution order, and the second sibling's
  `#[cfg(test)] mod helpers;` still composes.

## Test Mapping

- `crates/ripr/src/analysis/facts/role_composition.rs` — composition,
  crate-root, cycle, include-context, and disclosure-triggering fixture
  families.
- `crates/ripr/src/analysis/facts/cfg_predicates/tests.rs` — conditional
  path detection and the unlexable-`cfg_attr` fail-closed controls.
- `crates/ripr/src/analysis/facts/includes.rs` — include producer and
  conflicting-requirement tests.
- `crates/ripr/src/analysis/syntax/ra.rs` — module-declaration producer and
  include-directive producer tests.
- `crates/ripr/src/analysis/rust_index.rs` — module composition disclosure
  test.

## Implementation Mapping

- `crates/ripr/src/analysis/facts/role_composition.rs` — the composition
  pass (`compose_index_source_roles`, `ContextResolver`, `CrateRoots`).
- `crates/ripr/src/analysis/facts/model.rs` — `ModuleDeclarationFact`,
  `ModulePathTarget`, `ResolvedIncludeParent`, `SourceRoleProvenance`.
- `crates/ripr/src/analysis/facts/includes.rs` — include-edge resolution and
  compilation-unit rebasing.
- `crates/ripr/src/analysis/syntax/ra.rs` — module-declaration and
  include-directive producers.
- `crates/ripr/src/analysis/facts/cfg_predicates.rs` — the shared cfg
  attribute authority (test requirement, conditional `path` introductions).
- `crates/ripr/src/analysis/rust_index.rs` — the module composition
  disclosure.
- `crates/ripr/src/analysis/workspace/select.rs` — narrowed-mode selection
  pulls default-layout module parents so the composition edges can resolve.

## Metrics

- `unit_test_pass_rate` and `golden_fixture_pass_rate` cover the composition
  behavior; `cargo xtask goldens check` must stay green with zero drift.
