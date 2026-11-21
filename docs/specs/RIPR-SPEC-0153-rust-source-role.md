# RIPR-SPEC-0153: Rust producer-owned source role

Status: accepted

Ratified 2026-09-01: the #3534 conformance corpus and
`cargo xtask check-rust-source-role-authority` passed on #3618 after
the implementation train (#3499, #3519 disposition, #3530-#3534) merged.

Reworked 2026-09-03 (#3634): the harness-target validation sources
workspace membership and the test-target inventory from `cargo
metadata` itself, replacing the bounded manifest TOML emulation.

Reworked 2026-09-04 (#3631): the authority gate's cfg-test regions are
derived from the real grammar (`ra_ap_syntax`) instead of a hand-written
depth-tracking scanner, eliminating the scanner's documented edge cases
(else-if initializer chains, multi-line attribute/doc trim gaps, braced
const-generic arguments) and a differential-run finding where escaped
character literals leaked whole cfg-test modules into scans. Files that
do not parse cleanly are scanned verbatim (over-catch only) and the
fallback is disclosed in `source-role-authority.md`.

Issue: #3283 (parent #3213; builds on #3273 and #3286)

## Problem

Source role was derived independently by each consumer from path
fragments: `rust_index::is_test_file` covered `tests/**` for the diff
path, `workspace::is_production_rust_path` excluded `benches/` and
`examples/` for the repo path, and Cargo target declarations were never
read at all. A changed Cargo bench seeded production probes for harness
plumbing (`criterion_group!`, `criterion_main!`), an explicit `[[test]]`
target outside `tests/` could not confirm evidence role, and a confirmed
`*_test.rs` filename convention was indistinguishable from an ordinary
production file.

## Behavior

One typed, producer-owned role model (`SourceRole` in
`analysis/workspace/source_role.rs`) classifies each Rust file:

```text
production_subject                ordinary production: seeds probes/seams
test_evidence                     tests/** layout or declared [[test]] target
bench_evidence                    benches/** layout or declared [[bench]] target
example_evidence                  examples/** cargo-discoverable shapes
fixture_or_receipt_evidence       fixtures/, target/, .git/, .ripr/, ...
production_like_test_infrastructure  explicit opt-in target
unknown_role                      reserved for ambiguous inputs; test-only
```

Role is derived from authoritative context in priority order:

1. the `[analysis] production_like_targets` opt-in in `ripr.toml`
   (workspace-relative paths; absolute or backslashed values fail closed
   at parse time) — restores ordinary production analysis for the
   selected target only;
2. declared Cargo targets: explicit `path = ...` entries on `[[test]]`
   and `[[bench]]` confirm evidence role outside the default layouts.
   Autodiscovered targets live under `tests/`/`benches/` by construction,
   so `autotests = false` / `autobenches = false` need no separate
   handling; malformed manifests yield no targets and layout keeps
   applying (fail-closed toward production);
3. package layout: `tests/` anywhere and a `tests.rs` stem (matching
   the pre-existing contracts); `xtask/`, non-source directories, and
   files without a `src` component (the full pre-#3283 repo production
   contract — routing the repo set through the role cannot widen it);
   `benches/` and `examples/` only in Cargo-autodiscovery shapes
   (`<dir>/<name>.rs`, `<dir>/<name>/main.rs`) — nested src layouts like
   `examples/sample/src/lib.rs` are NOT discoverable targets and stay
   production subjects;
4. everything else is a production subject. A filename convention alone
   (`*_test.rs`, `test_*.rs`) never classifies.

Both diff probe seeding and the repo seam-inventory production set route
through `classify_with`; `benches/**`/`examples/**` harness plumbing no
longer seeds production obligations, closing the diff gap while the repo
exclusion stays consistent. Evidence-role files remain fully indexed:
functions stay available for owner relations, activation input,
sink/oracle evidence, and selectors. `TestFact` semantics are untouched
— source role never registers a helper as an executable test selector.

At item granularity, the typed `FunctionSourceRole` model (#3531)
classifies each function: `Production`, `TestAttribute` (exact
supported test-defining attribute; registers an executable
`TestFact`), `CfgTestModule` (evidence-only member of a
test-required module, preserved by the facts normalizer and the
#3533 composition walk), `HarnessHelper` (demoted member of a
registered `harness = false` target), and
`RegisteredTestAttribute` (promoted through a repository-
governed `[analysis.test_harnesses]` registration, #3532), and
`ParameterizedExpansion` (expansion of a parameterized-test macro
registration; registers executable tests).
Attribute-driven membership is authoritative; filenames, imports,
and macro suffixes never classify.

Diff lines without a function owner consume the composer's existing
`SourceRoleProvenance` (#3695). They are evidence-only when the provenance
has no unresolved edge and contains a test-required edge, and the child
has parser-backed facts. Empty, missing, production, or unresolved
provenance and lexical-fallback children retain production eligibility;
function-owned lines retain their `FunctionSourceRole` decision. This
includes field declarations in regular, literal-path, and transitive
test-required module children. It does not register executable tests or
change the contextual-role producer.

A `custom_harness` registration grants its file-wide evidence role
(and the registry's helper demotion and trial-subject derivation)
only after its target validates against the workspace's Cargo
target metadata (#3608; metadata-sourced since #3634): the target
must match a declared `[[test]]` target — explicit `path = ...` or a
name-only entry resolved to its autodiscovery shape — whose effective
`harness` flag is `false`. Membership and target identity come from
`cargo metadata` itself: each batch runs one bounded
`cargo metadata --no-deps --offline` probe against the analysis root
(the child cwd is anchored there, so a caller running inside another
workspace cannot have the probe rejected as a foreign manifest), and
its `packages[].targets[]` inventory is the authority for both the
member set and the test-target list. That resolves exactly the shapes
the previous manifest TOML emulation approximated or dropped:
`[workspace.dependencies]` inheritance (an inherited path dependency is
a member), character-class member globs (`crates/[ab]` expands),
`[workspace.exclude]` (cargo matches exclude patterns as literal path
prefixes — a wildcard component matches no member, a bare `dep` prefix
excludes the subtree, and a literal member entry beats its
parent-prefix exclusion), and regular/dev/build path dependencies. The
exclude x path-dependency precedence is contested upstream; cargo
resolves it exclude-wins on the pinned toolchain (verified empirically,
direct and nested), which is also the under-credit direction, so the
behavior is pinned rather than chosen. The `harness` flag is absent
from metadata output by construction (verified on the pinned
toolchain), so the flag premise still comes from parsing the owning
package manifest's `[[test]]` entries — explicit path spellings and
name-only `tests/<name>.rs` defaults alike; cargo keeps declared `..`
segments in `src_path` as spelled, so declared paths and registration
targets resolve lexically on both sides. Conflicting flags across
owning manifests — the same path claimed by two packages — are
ambiguous ownership and fail closed. A target missing from the
inventory is `target_not_declared`; a target present without any naming
manifest entry is autodiscovery, whose libtest default
(`harness = true`; cargo resolves edition, `autotests`, and the
directory-versus-file layouts itself) is `harness_flag_conflict`. Any
unresolvable state — no cargo binary, a workspace cargo rejects (a
broken member manifest, a virtual manifest carrying a target table, a
glob member without a manifest), a probe that outlives its deadline, or
an unreadable probe output — yields `manifest_unavailable`: the
registration grants nothing, never over-credits, and the file keeps
its ordinary classification, so a misdeclared target that ordinary
classification treats as production keeps seeding production seams
while a target in a `tests/`-style or Cargo-discovered evidence
layout keeps its evidence role. Every file-wide evidence surface (diff
seeding, repo seam inventory, LSP scope partition) consumes the same
validated grant, so the degradation is identical everywhere, and the
classified-seam caches bump their schema generations so pre-#3634
entries cannot serve classifications that bypass the validation.

The opt-in joins the check-artifact config identity as
FindingAffecting (`CHECK_ARTIFACT_CONFIG_IDENTITY_VERSION` 1 → 2) and
the repo-exposure consumed-config list; the workspace cache key already
hashes the `ripr.toml` text, so no generation bump is required for it.
The #3532 harness registry joined the same identity as FindingAffecting
(2 → 3, canonical length-prefixed encoding pinned byte-for-byte).

## Required Evidence

- A changed Cargo bench seeds no production probes but stays in
  changed-file accounting: pinned by the `benches_harness_evidence`
  corpus fixture (four bench findings on main, zero after the slice) and
  an in-crate regression test verified failing on main.
- A declared `[[test]] path="src/contract_test.rs"` target's plain
  harness helper seeds no production probes while the same-shaped
  undeclared `src/unconfirmed_test.rs` stays a production subject;
  disabling the declared-target branch makes the fixture fail
  (discriminating-power proof).
- The opt-in restores production analysis for the selected target only;
  sibling test targets stay evidence-only.
- Layout pins: nested `examples/<dir>/src/**` and
  `benches/<dir>/src/**` are production; `<dir>/<name>.rs` and
  `<dir>/<name>/main.rs` are evidence; `tests/` any-segment remains
  evidence; `src/test_helper.rs` remains production.
- Manifest parsing pins: explicit paths extracted, entries without
  `path` contribute nothing, malformed TOML yields no targets,
  autodiscovery flags do not drop explicit targets.
- Config pins: the opt-in parses as relative paths, joins the identity
  canonically, and absolute paths fail closed with a named error.
- `#3273`'s inline `#[cfg(test)]` controls and `#3286`'s helper-evidence
  regression tests remain green.

- The repo production contract carries over exactly: `xtask/`, files
  without a `src` component, and `tests.rs` stems stay non-production;
  the single declared divergence is nested-src layouts under
  `examples/`/`benches/` (not Cargo-discoverable targets, production
  in diff mode since before #3283).

## Required guards


- No filename-only classification: an unconfirmed convention name is a
  production subject.
- The opt-in never leaks to sibling targets.
- Evidence files stay indexed; no evidence path is dropped from the
  index or from related-test discovery.
- `TestFact` registration remains attribute-driven only.
- A `custom_harness` registration never grants file-wide evidence role
  without a confirmed `harness = false` Cargo declaration (#3608): a
  misdeclared target keeps its ordinary classification — targets that
  ordinary classification treats as production keep seeding production
  seams, while `tests/`-style or Cargo-discovered evidence layouts stay
  evidence-only — and the conflict is recorded as a typed limitation
  naming the target.

## Acceptance Examples

- Accept: `benches/exposure.rs` changed → indexed, counted as a changed
  file, zero production findings.
- Accept: `[[test]] path="src/contract_test.rs"` → helper in it is
  evidence; `src/unconfirmed_test.rs` without a declaration → production.
- Accept: `production_like_targets = ["tests/api_contract.rs"]` → that
  file analyzed as production; `tests/other.rs` stays evidence.
- Accept: a `custom_harness` registration on a declared
  `harness = false` target (explicit custom path or name-only entry on
  the conventional layout) → file-wide evidence role with adapter
  subjects; the same registration against an undeclared or
  `harness = true` target → typed limitation, no demotion, and a
  production-classified target keeps seeding production seams while an
  evidence-layout target keeps its evidence role (#3608).
- Reject: a bench harness call (`criterion_main!`) becoming an
  obligation; a `test_*.rs` name alone excluding a production file.

## Test Mapping

`analysis/workspace/source_role.rs` unit tests pin the layout rules,
Cargo-autodiscovery shapes, override priority, windows normalization,
and the seeding/evidence partition. `analysis/workspace/cargo_targets.rs`
pins manifest extraction and the #3608 harness-target Cargo verdict
(declared, harness-enabled, undeclared, manifest-unavailable) against
real `cargo metadata` workspaces (#3634): metadata-sourced membership
(workspace-inherited path dependencies, character-class member globs,
wildcard exclude patterns, exclude x path-dependence precedence,
recursive member globs), the fail-closed probe boundary, and the
manifest-sourced `harness` flag.
`analysis/facts/harness_registry` pins the conflict limitations and the
degraded per-function behavior for misdeclared targets, and
`analysis/language/rust.rs` pins the diff-path seeding flip alongside
diff seeding
(bench gap regression, declared-target confirmation with a probeable
helper, opt-in restore). `config/tests.rs` pins parsing, identity
classification, and absolute-path rejection.
`analysis::source_role_corpus` loads the retained conformance corpus
(`crates/ripr/tests/data/source-role-corpus/cases/`) and pins
executable-test membership, layout classification, naming-lookalike
rejection, and cfg-variant equivalence against `facts::build_index`.
`cargo xtask check-rust-source-role-authority` structurally rejects
consumer-side role re-derivation and inventories the approved
`rust_index::is_test_file` consumers. Its production regions come from
the parser-backed authority (`xtask/src/rust_region_scan.rs`, #3631);
the honored cfg-test spelling stays exactly `cfg(test)` so the exemption
inventory is unchanged, and a parse failure degrades to a disclosed
verbatim scan instead of a second lexical authority.

## Non-Goals

- No assertion-form parity (#3284 owns it).
- No public output/gate role projection (#3285 owns it).
- No cache-generation bump: the workspace cache key hashes the
  `ripr.toml` text, and the fact/classification schemas are unchanged by
  this slice (`FunctionFact.is_test` semantics untouched).
- Custom-harness classification now exists only through the explicit
  repository-governed registry (`[analysis.test_harnesses]`, #3532):
  exact registrations authorize evidence role, adapter-established
  subjects, and typed limitations; nothing is inferred from filenames,
  imports, macro suffixes, or function names. Generated-file
  classification without a real producer remains deferred.

## Implementation Mapping

- `analysis/workspace/source_role.rs` — the typed file-role model.
- `analysis/facts/` — item-role producers: the facts builders, the
  cfg-predicate authority (`cfg_predicates.rs`), the harness registry
  (`harness_registry.rs`, #3532), role composition
  (`role_composition.rs`, #3533), and the test-style normalizers.
- `analysis/source_role_corpus.rs` — the conformance corpus driver.
- `analysis/harness_projection.rs` — the typed harness projection.
- `analysis/workspace/cargo_targets.rs` — manifest enumeration,
  workspace-root-anchored.
- `analysis/language/rust.rs` — diff seeding and repo production set.
- `analysis/seam_inventory.rs` — inventory and count production sets.
- `config.rs` + `config/model.rs` — the opt-in, its identity role, and
  the consumed-config list.

## Metrics

No new metric; existing counts now exclude bench/example harness
plumbing from production denominators by the same authority.
