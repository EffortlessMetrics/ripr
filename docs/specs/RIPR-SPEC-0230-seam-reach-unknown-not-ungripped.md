# RIPR-SPEC-0230: Seam reach unknown, not ungripped

Status: proposed

Owner: product-analysis

Created: 2026-10-04

Linked issues: #5411 (pilot ranks unresolved reach as the top gap), #5295
(mutation spot check), #5334 (crediting that reach, out of scope here).
The `ripr check` side of the same rule is #5416. Numbered 0230 because
#5512 names RIPR-SPEC-0232, RIPR-SPEC-0233, and RIPR-SPEC-0227 through0229 in the combined source index.

Support-tier impact:

- No tier change. `opaque` is an existing grip class; no schema value,
  JSON field or `schema_version` changes. Seams that read `ungripped`
  because reach was unresolved now read `opaque` and leave the headline
  gap count. [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact: none. No new process, network, file-policy or dependency
surface. The classified-seam cache generations move (`1.31`, `0.37`).

## Problem

A repo seam with no related test got reach `no`, and RIPR-SPEC-0005 rule 1
maps reach `no` to `ungripped`, the class `ripr pilot` ranks first and the
LSP reports as "No detected test grip". But "no related test" is often an
analyzer limit, not an established negative. ripr does not trace calls
through `pub` to `pub(crate)` helper chains past the direct-call relation,
through macros, or through trait dispatch (`to_string()` running
`Display::fmt`, `<` running `PartialOrd::partial_cmp`). The mutation spot
check (#5295) at semver `280ebcb6edac` found 22 of 23 seam-precise
`ungripped` mutants caught by the crate's tests. All 12 seams sit in
`Display`/`Ord`/`FromStr` impls or private parse helpers that integration
tests reach through the public API.

`ripr check` already names these limits on `no_static_path` findings
(RIPR-SPEC-0114, RIPR-SPEC-0118). The seam surface ignored them.

## Behavior

The rule: a seam is `ungripped` only when ripr established the negative.
When a seam has no related test, the evidence producer looks for an
unresolved candidate path, in this order:

1. A bounded transitive witness (RIPR-SPEC-0114/0118): a test calls an
   in-crate entry point whose name-matched call chain reaches the owner's
   name within `MAX_TRANSITIVE_DEPTH` hops.
2. A macro-reach witness (the existing `no_static_path` macro check).
3. Trait dispatch. The owner is a trait-impl method (`impl Trait for T`),
   and a test body, or production code a test may run, names `T`.
4. Trait dispatch, one level down. The owner is called, within the bounded
   walk, by a trait-impl method whose self type test-reached code names.
   This includes a trait method whose own type nothing names but which a
   named type's trait method delegates to (`self.inner.fmt(f)`).
   Each such method's callees join the test-reached set, so a type they
   name can root a further method, up to `MAX_TRANSITIVE_DEPTH` rounds.

"Production code a test may run" is the forward closure, by name over
non-macro call facts, of every test's callees within `MAX_TRANSITIVE_DEPTH`
hops. A mention is a capitalized identifier outside comments and string
literals.

When any of these fires, reach is `opaque` (confidence low) and its summary
names the limit and the witness, for example:

```text
Reach unresolved (rust_integration_public_api_path_unresolved): `test_new`
(tests/test_identifier.rs:14) calls `new`, which may lead to `identifier`
through a call path ripr does not fully trace; no gap is reported
```

The transitive and macro summaries carry the existing `static_limit_kind`
wire value that `ripr check` emits for the same witness. Trait dispatch has
no `static_limit_kind` value and reads `(trait dispatch)`.

RIPR-SPEC-0005 rule 2 (any stage `opaque` gives `opaque`) then classifies
the seam `opaque`. It is not headline-eligible, pilot ranks it after every
gap class, the LSP shows it at information severity, and its evidence
record carries the reach summary as an `opaque_static_evidence` limitation.

When none fires, reach stays `no` and the seam stays `ungripped`.

Known limits, both in the fail-closed direction (the seam stays
`ungripped`, or reads `opaque` where it might not need to):

- Only capitalized identifiers count as type mentions, so a trait impl for a
  primitive or lowercase self type (`impl Encode for u32`) never meets rule 3.
- Comments and strings are stripped line by line, so a type named only inside
  a block comment or multi-line string still counts as a mention.
- Matching is by name, as in RIPR-SPEC-0114. A test that calls a common name
  such as `new` or `parse` pulls in every same-named function, so rule 3
  covers most trait impls of types a crate's tests use, including `Debug`,
  `Hash` or `Drop` impls the tests never exercise. Those seams read unknown,
  not gripped.

The witness is a candidate path. It never becomes a related test and adds
no reach, activation, propagation, observation or discrimination credit.
Seams with related tests are unchanged.

## Non-Goals

- Crediting the unresolved reach (#5334). The seam reads unknown, not
  gripped.
- Changing `ripr check` findings (#5416 owns that surface).
- A new grip class or `static_limit_kind` value.
- Tracing trait dispatch, generics or macros precisely.

## Required Evidence

- A seam whose owner no test path reaches stays `ungripped` (reach `no`).
- A private helper behind a public function an integration test calls is
  `opaque` and names the transitive witness.
- A trait-impl method of a type a test uses is `opaque` and names the test.
- A trait-impl method of a type no test-reached code names stays
  `ungripped`.
- A helper run only from a trait-impl method of a test-used type is
  `opaque` and names the dispatch root.
- Runtime-control integration tests (`cargo test -p ripr --test '*'`) pass
  unchanged.

## Acceptance Examples

At semver `280ebcb6edac`, `ripr check --format repo-exposure-json`:

| Grip class | Before | After |
| --- | --- | --- |
| `ungripped` | 505 | 1 |
| `opaque` | 0 | 504 |

The 504 moves: 321 transitive witnesses, 183 trait dispatch. The spot check
scored 12 semver seams (23 mutants). Each of the 12 had at least one mutant
the crate's tests caught, and all 12 now read `opaque`. Its one missed mutant
sits on one of those same seams, so no seam whose mutants all survived
changed class. The one remaining `ungripped` seam is
`impl Version::cmp_precedence`.

On the other spot-check repositories: rust-hex and strsim-rs have no
`ungripped` seams before or after. bytesize moves 2 of 5 (trait dispatch);
its 3 `ensure-no-std` seams, including a `GlobalAlloc` impl no test reaches,
stay `ungripped`. humantime moves 8 of 8 (trait dispatch).

## Test Mapping

- `crates/ripr/src/analysis/test_grip_evidence/tests.rs::seam_reached_by_no_test_path_stays_ungripped`
- `crates/ripr/src/analysis/test_grip_evidence/tests.rs::seam_behind_an_unresolved_transitive_path_is_opaque_not_ungripped`
- `crates/ripr/src/analysis/test_grip_evidence/tests.rs::trait_method_of_a_type_tests_use_is_opaque_not_ungripped`
- `crates/ripr/src/analysis/test_grip_evidence/tests.rs::trait_method_of_a_type_no_test_reaches_stays_ungripped`
- `crates/ripr/src/analysis/test_grip_evidence/tests.rs::helper_run_only_through_trait_dispatch_is_opaque_not_ungripped`
- `crates/ripr/src/analysis/test_grip_evidence/tests.rs::trait_method_reached_only_by_delegation_is_opaque_not_ungripped`
- `crates/ripr/src/analysis/test_grip_evidence/reach_limit.rs::tests::identifiers_keep_type_shaped_tokens_outside_comments_and_strings`
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::transitive_reach_limit_kind_names_integration_test_path`
- `crates/ripr/src/analysis/classify/transitive_reach.rs::tests::macro_reach_limit_kind_names_direct_test_body_macro_path`

## Implementation Mapping

- `crates/ripr/src/analysis/test_grip_evidence/reach_limit.rs`: the rule,
  the type-mention and dispatch-root index, and the reach summaries
- `crates/ripr/src/analysis/test_grip_evidence.rs`: `reach_evidence` calls
  it for full and compact evidence
- `crates/ripr/src/analysis/test_grip_evidence/related_tests/context.rs`:
  the run-scoped transitive index, type-mention index and per-owner cache
- `crates/ripr/src/analysis/classify/transitive_reach.rs`: forward
  test-reached closure and the shared witness-to-`static_limit_kind` mapping
- `crates/ripr/src/analysis/seam_cache.rs`: cache generations

## Metrics

- `ungripped` seams whose reach rests on an unresolved path: 0 by
  construction
- semver `ungripped`: 505 before, 1 after
