# RIPR-SPEC-0095: TypeScript Single-Hop Re-Export Test Discovery

Status: accepted

## Problem

ripr reported `no_static_path` for tests that genuinely exercise a changed
TypeScript function but import it through a barrel-file re-export (e.g.,
`src/index.ts` re-exports `isRawNetworkError` from `src/util.ts`). This is
the single biggest cause of false `no_static_path` on real TypeScript repos —
barrel-file re-exports are the standard public API pattern in TypeScript.

Concrete repro (Ky-style):

```
src/util.ts      exports function isRawNetworkError(error: unknown): boolean
src/index.ts     export { isRawNetworkError } from './util'
test/util.test.ts  import { isRawNetworkError } from '../src/index'
                   expect(isRawNetworkError(new Error())).toBe(true)
```

Before this spec: `no_static_path`, 0 related tests.
After: `exposed`, 2 related tests, `relation_reason: re_export_chain_followed`.

## Behavior

### Single-hop re-export tracing

When checking test → owner, the adapter MUST follow ONE explicit named
re-export hop:

```
test file imports N from file B
  →  file B has `export { N } from './A'` or `export { N as M } from './A'`
  →  file A contains the changed owner function
```

If the chain resolves to the changed owner in ONE hop, the test is credited.

The `ReExportIndex` is built in Phase 1 (alongside `all_owners`, `all_tests`).
It is a `HashMap<(intermediate_module, exported_name), (original_name, source_module)>`.
The index is O(1) to query per import during test-matching.

### Fail-closed bounds

- Two-hop and deeper chains MUST NOT be followed (fail-closed). Only explicit,
  single-hop, in-source re-exports are resolved.
- Star re-exports (`export * from`) are NOT indexed (too broad, fail-closed).
- Non-relative module specifiers (package names, node_modules) are NOT followed.
- Default re-exports are NOT followed (out of scope for this slice).
- Namespace imports through re-export chains are NOT followed.
- Type-only re-exports (`export type { N }`) are NOT indexed.

### Honesty bar

The re-export chain MUST resolve to both the correct source file AND the
correct function name. A same-name function in a different file that is
re-exported by the same barrel MUST NOT be credited (name-collision guard).

### Disclosure

When a test is credited via a re-export chain, the `RelatedTest` entry MUST
carry:

```json
"relation_reason": "re_export_chain_followed",
"relation_confidence": "medium"
```

The `medium` confidence reflects that the chain is explicit in-source but
involves one level of indirection that ripr cannot verify at runtime.

## Required Evidence

- Fixture `typescript_reexport_single_hop`: owner changed, single-hop barrel
  re-export, test imports from barrel → must produce `exposed`,
  `relation_reason: re_export_chain_followed`.
- Fixture `typescript_reexport_no_false_credit`: barrel re-exports same name
  from a DIFFERENT file → must stay `no_static_path`, 0 related tests.
- Fixture `typescript_reexport_two_hop_limit`: two-hop chain → must stay
  `no_static_path`, 0 related tests (fail-closed).
- `cargo xtask goldens check` must pass for all three fixtures.
- `RUSTFLAGS="-D warnings" cargo build -p ripr -p xtask` must pass.

## Non-Goals

- Transitive multi-hop chains beyond one hop.
- Star re-exports (`export * from`).
- Default re-export tracing.
- Package-graph resolution (no node_modules, no tsc, no package.json lookups).
- Namespace import tracing.

## Acceptance Examples

### Single-hop credit (must credit via re-export chain)

```
owner:              isRawNetworkError in src/util.ts
barrel:             src/index.ts — export { isRawNetworkError } from './util'
test import:        import { isRawNetworkError } from '../src/index'
test assertion:     expect(isRawNetworkError(e)).toBe(true)

Before fix:         no_static_path, 0 related tests
After fix:          exposed, 2 related tests,
                    relation_reason: re_export_chain_followed,
                    relation_confidence: medium
```

### No-false-credit control (must stay no_static_path)

```
owner:              isRawNetworkError in src/util.ts
barrel:             src/index.ts — export { isRawNetworkError } from './other'
test import:        import { isRawNetworkError } from '../src/index'

After fix:          no_static_path, 0 related tests
                    (chain resolves to src/other.ts, not src/util.ts)
```

### Two-hop limitation control (must stay no_static_path)

```
owner:              isRawNetworkError in src/util.ts
chain:              index.ts → errors.ts → util.ts  (two hops)
test import:        import { isRawNetworkError } from '../src/index'

After fix:          no_static_path, 0 related tests
                    (only one hop resolved: index.ts → errors.ts, not owner)
```

## Test Mapping

- `fixtures/typescript_reexport_single_hop/` — golden fixture for single-hop credit.
- `fixtures/typescript_reexport_no_false_credit/` — golden fixture for name-collision guard.
- `fixtures/typescript_reexport_two_hop_limit/` — golden fixture for two-hop fail-closed.
- `crates/ripr/src/analysis/language/typescript/tests.rs` — unit tests for
  `ReExportIndex::build`, `ReExportIndex::resolve_to_owner`, and
  `find_related_tests` with re-export index.
- `crates/ripr/src/domain/evidence.rs` — `relation_reason_labels_are_stable_contract_terms`
  includes `ReExportChainFollowed`.

## Implementation Mapping

- `crates/ripr/src/domain/evidence.rs`: `RelationReason::ReExportChainFollowed` variant.
- `crates/ripr/src/analysis/language/typescript/types.rs`:
  `TypeScriptRelationKind::ReExportChainFollowed` variant.
- `crates/ripr/src/analysis/language/typescript/related_tests.rs`:
  - `ReExportIndex` struct with `empty()`, `build()`, `resolve_to_owner()`.
  - `owner_call_relation` — extended to check single-hop re-export after existing checks.
  - `find_related_tests` — calls `ts_relation_to_domain()` to populate
    `relation_reason` and `relation_confidence` for all relation kinds.
  - `ts_relation_to_domain()` — new mapping fn.
- `crates/ripr/src/analysis/language/typescript/classifier.rs`:
  `classify_change` accepts `reexport_index` parameter.
- `crates/ripr/src/analysis/language/typescript/mod.rs`:
  builds `ReExportIndex` in Phase 1 and passes it to `classify_change`.

## Metrics

- `typescript_reexport_single_hop_credits_via_barrel`: fixture
  `typescript_reexport_single_hop` produces `exposed` with at least 1 related
  test carrying `relation_reason: re_export_chain_followed` (validated by
  `cargo xtask fixtures typescript_reexport_single_hop`).
- `typescript_reexport_no_false_credit_stays_no_path`: fixture
  `typescript_reexport_no_false_credit` produces `no_static_path` with 0
  related tests (validated by `cargo xtask fixtures typescript_reexport_no_false_credit`).
- `typescript_reexport_two_hop_stays_no_path`: fixture
  `typescript_reexport_two_hop_limit` produces `no_static_path` with 0
  related tests (validated by `cargo xtask fixtures typescript_reexport_two_hop_limit`).
