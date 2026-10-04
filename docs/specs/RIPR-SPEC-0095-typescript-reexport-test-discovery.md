# RIPR-SPEC-0095: TypeScript Bounded Re-Export Test Discovery

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

Second repro (unjs/ufo eb29945 / 5cd9e67): the test imports through the
package DIRECTORY, whose index is a star barrel:

```
src/utils.ts       export function withoutBase(input, base) { ... }
src/index.ts       export * from "./utils";  (plus other `export *` lines)
test/base.test.ts  import { withBase, withoutBase } from "../src";
```

The first slice resolved `../src` to module `src`, which matched no barrel, so
every changed line read `no_static_path` with 0 related tests.

## Behavior

### Bounded re-export tracing

When checking test → owner, the adapter resolves the name the test imports
through the in-repo re-export graph and credits the test when the chain lands
on the changed owner's own export:

```
test file imports N from module B
  →  B: `export { N } from './A'`, `export { M as N } from './A'`,
        or `export * from './A'` (A must itself export N)
  →  ... at most MAX_REEXPORT_HOPS (4) hops ...
  →  owner module: declares and exports the changed owner as N
                   (`export function N`, `export const N`, `export { N }`,
                   `export { local as N }`, or `export default` reached
                   through `export { default as N } from`)
```

Module specifiers resolve relative to the importing file (or through tsconfig
`paths` when that option is enabled). A directory specifier (`'../src'`,
`'../src/'`) resolves to `src/index` only when no `src.ts`-style file module of
the same name exists — the TypeScript/Node lookup order. A test that imports
the owner directly through such a directory specifier (owner declared in
`src/utils/index.ts`, test imports from `'../src/utils'`) is credited through
the same relation.

The `ReExportIndex` is built in Phase 1 (alongside `all_owners`, `all_tests`)
from the non-test workspace sources. Per module it records named re-exports
`(module, exported_name) → (original_name, source_module)`, the module's own
rename exports as self-hops, `export *` edges, the names the module exports
from its own declarations, and the set of known workspace modules.

### Fail-closed bounds

- A chain longer than `MAX_REEXPORT_HOPS` (4) hops is not followed.
- A re-export cycle contributes no binding and terminates.
- A star hop forwards a name only when the target module (or its bounded star
  closure) exports it; a module-private function is never forwarded.
- A name exported by two star sources with different bindings is ambiguous
  and not credited.
- `export *` never forwards `default`.
- A local export of the name in an intermediate module shadows its star
  sources.
- `export * as ns from`, type-only re-exports (`export type { N } from`,
  `export type * from`), and non-relative package specifiers (node_modules)
  are NOT followed.
- Default imports and namespace imports through re-export chains are NOT
  followed.
- A test body that redeclares the imported local name (`const N = ...`) calls
  the shadow, not the owner, and is not credited.

### Honesty bar

The re-export chain MUST resolve to both the correct source file AND the
correct function name. A same-name function in a different file that is
re-exported by the same barrel MUST NOT be credited (name-collision guard).
A test that imports only a different name from the barrel that forwards the
changed owner MUST NOT be credited: sharing a barrel is not a relation.

### Disclosure

When a test is credited via a re-export chain, the `RelatedTest` entry MUST
carry:

```json
"relation_reason": "re_export_chain_followed",
"relation_confidence": "medium"
```

The `medium` confidence reflects that the chain is explicit in-source but
involves indirection that ripr cannot verify at runtime.

## Required Evidence

- Fixture `typescript_reexport_single_hop`: owner changed, single-hop barrel
  re-export, test imports from barrel → must produce `exposed`,
  `relation_reason: re_export_chain_followed`.
- Fixture `typescript_reexport_no_false_credit`: barrel re-exports same name
  from a DIFFERENT file → must stay `no_static_path`, 0 related tests.
- Fixture `typescript_reexport_two_hop_limit`: two-hop named chain within the
  hop bound → must produce `exposed`, `relation_reason: re_export_chain_followed`.
- Fixture `typescript_reexport_directory_star_barrel`: the ufo shape (directory
  import of a star barrel) → must produce `exposed`,
  `relation_reason: re_export_chain_followed`.
- Fixture `typescript_reexport_barrel_other_name`: the test imports only
  another name from the barrel that forwards the changed owner → must stay
  `no_static_path`, 0 related tests.
- Unit tests in `reexport_chain_tests` for the over-bound chain, cycle,
  ambiguous star, non-exported star target, file-module-over-index, and
  shadowed import controls.
- `cargo xtask goldens check` must pass for all fixtures.

## Non-Goals

- Chains beyond `MAX_REEXPORT_HOPS`.
- Following a module's own `import { x } from './a'; export { x }` re-binding
  (under-credited, fail-closed).
- Default-import and namespace-import tracing through barrels.
- Package-graph resolution (no node_modules, no tsc, no package.json
  `exports`/`main` lookups).

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

### Two-hop chain (credited within the bound)

```
owner:              isRawNetworkError in src/util.ts
chain:              index.ts → errors.ts → util.ts  (two named hops)
test import:        import { isRawNetworkError } from '../src/index'

After:              exposed, 1 related test,
                    relation_reason: re_export_chain_followed
```

### Directory star barrel (unjs/ufo shape)

```
owner:              withoutBase in src/utils.ts
barrel:             src/index.ts — export * from './url'; export * from './utils'
test import:        import { withoutBase } from '../src'

Before:             no_static_path, 0 related tests
After:              exposed, 2 related tests,
                    relation_reason: re_export_chain_followed
```

### Barrel other-name control (must stay no_static_path)

```
owner:              applyDiscount in src/a.ts
barrel:             src/index.ts — export { applyDiscount } from './a'; export * from './b'
test import:        import { formatCents } from '../src'

After:              no_static_path, 0 related tests
```

## Test Mapping

- `fixtures/typescript_reexport_single_hop/` — golden fixture for single-hop credit.
- `fixtures/typescript_reexport_no_false_credit/` — golden fixture for name-collision guard.
- `fixtures/typescript_reexport_two_hop_limit/` — golden fixture for a
  two-hop chain within the bound.
- `fixtures/typescript_reexport_directory_star_barrel/` — golden fixture for
  the directory star barrel.
- `fixtures/typescript_reexport_barrel_other_name/` — golden fixture for the
  barrel other-name control.
- `crates/ripr/src/analysis/language/typescript/tests/reexport_chain_tests.rs`
  — `ReExportIndex::build` + `find_related_tests` over real sources for
  positive chains and every fail-closed bound.
- `crates/ripr/src/analysis/language/typescript/tests.rs` — unit tests for
  `ReExportIndex::from_parts`, `ReExportIndex::resolve_to_owner`, and
  `find_related_tests` with re-export index.
- `crates/ripr/src/domain/evidence.rs` — `relation_reason_labels_are_stable_contract_terms`
  includes `ReExportChainFollowed`.

## Implementation Mapping

- `crates/ripr/src/domain/evidence.rs`: `RelationReason::ReExportChainFollowed` variant.
- `crates/ripr/src/analysis/language/typescript/types.rs`:
  `TypeScriptRelationKind::ReExportChainFollowed` variant.
- `crates/ripr/src/analysis/language/typescript/related_tests.rs`:
  - `ReExportIndex` struct with `empty()`, `from_parts()`, `build()`,
    `resolve_to_owner()`, and the bounded `resolve_export()` walk;
    `MAX_REEXPORT_HOPS`.
  - `owner_call_relation` — checks the re-export chain after the direct and
    import relations, with the body-local shadow guard.
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
- `typescript_reexport_two_hop_credits_within_bound`: fixture
  `typescript_reexport_two_hop_limit` produces `exposed` with 1 related test
  carrying `relation_reason: re_export_chain_followed` (validated by
  `cargo xtask fixtures typescript_reexport_two_hop_limit`).
- `typescript_reexport_directory_star_barrel_credits`: fixture
  `typescript_reexport_directory_star_barrel` produces `exposed` with related
  tests carrying `relation_reason: re_export_chain_followed`.
- `typescript_reexport_barrel_other_name_stays_no_path`: fixture
  `typescript_reexport_barrel_other_name` produces `no_static_path` with 0
  related tests.
