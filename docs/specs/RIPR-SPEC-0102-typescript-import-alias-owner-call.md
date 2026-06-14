# RIPR-SPEC-0102: TypeScript Import-Alias Owner-Call Confidence Upgrade

Status: accepted

Owner: product / swarm

Created: 2026-06-14

Linked issues:

- #1239

Linked PRs:

- None yet

Support-tier impact:

- Under-emit fix for TypeScript preview relation classification: the alias-rename
  import case (`import { X as local }` + `local(...)`) is upgraded from
  `ImportedOwnerCall` / `import_path_affinity` / Medium to
  `ImportAliasOwnerCall` / `direct_owner_call` / High.
  No exposure-class change; the test was already credited. Language status stays
  Preview; no tier change.
  Claim boundaries and tier labels remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, or LSP servers.
- One new `TypeScriptRelationKind` variant (`ImportAliasOwnerCall`) in
  `crates/ripr/src/analysis/language/typescript/types.rs`.
- No new wire names: the domain mapping reuses the existing
  `relation_reason: "direct_owner_call"` string (already in the output contract).
- No `schema_version` bump. The emitted `relation_reason` value is already valid
  in the schema; this spec promotes an existing case to the correct reason.
- Register this spec in `policy/doc-artifacts.toml` and `docs/specs/README.md`.

## Problem

When a test imports an owner via an explicit renaming alias —

```typescript
import { computeValue as cv } from '../src/compute';

test('alias call', () => {
    expect(cv(5)).toBe(10);
});
```

ripr ALREADY credits the relation (via `TypeScriptRelationKind::ImportedOwnerCall`),
but `ts_relation_to_domain` maps `ImportedOwnerCall` →
`RelationReason::ImportPathAffinity` / `RelationConfidence::Medium`.

That understates the evidence. The `as` rename is syntactically explicit AND the
test body verifiably calls the local alias — equivalent certainty to a
`DirectOwnerCall`. The test WAS already found; only the stated reason and
confidence are wrong.

### Why the alias case equals DirectOwnerCall certainty

| Signal | DirectOwnerCall | ImportAliasOwnerCall |
|---|---|---|
| Import from owner file confirmed | yes | yes |
| Imported name matches owner name | — (same file) | yes (import.imported == owner.name) |
| Local name explicitly bound to alias | n/a | yes (import.local != owner.name) |
| Body calls the local alias | yes | yes |
| Shadow guard applied | yes (owner_name_shadowed_by_unrelated_import) | yes (new: local_identifier_declared_in_test_body) |

## Behavior

When a test file contains a renaming named import from the owner's source file
(`import { OriginalName as local } from '../path/to/owner'`) and the test body
contains a call expression `local(...)`:

- ripr classifies the relation as `ImportAliasOwnerCall` (internal kind).
- `ts_relation_to_domain` maps this to `RelationReason::DirectOwnerCall` and
  `RelationConfidence::High` (same as a direct call or method receiver call).
- Shadow guard: if `local` is re-declared as a `const`, `let`, `var`, or
  `function` inside the test body, the alias-upgrade arm does NOT fire.
  The shadowed case falls through to the prior behavior (typically uncredited).

All other import forms are unchanged:

| Import form | Kind | Reason | Confidence |
|---|---|---|---|
| `import { X }` + `X(...)` | DirectOwnerCall | direct_owner_call | High |
| `import { X as local }` + `local(...)` (no shadow) | ImportAliasOwnerCall | direct_owner_call | High |
| `import { X as local }` + `local(...)` (local shadowed) | falls through | unchanged | unchanged |
| `import * as ns` + `ns.X(...)` | ImportedOwnerCall | import_path_affinity | Medium |
| `import { Y as local }` (Y ≠ X) + `local(...)` | not credited | — | — |

## Solution

### 1. New `TypeScriptRelationKind` variant

Add `ImportAliasOwnerCall` to
`crates/ripr/src/analysis/language/typescript/types.rs`:

- `rank()` = 5 (same as `DirectOwnerCall`)
- included in `uses_oracle()` (same as `DirectOwnerCall`)
- `as_str()` = `"import_alias_owner_call"` (internal diagnostic only; not
  emitted in public JSON output — the domain reason is `direct_owner_call`)

### 2. Split the `ImportedOwnerCall` arm in `owner_call_relation`

In `related_tests.rs::owner_call_relation`, before the existing
`ImportedOwnerCall` check, insert an `ImportAliasOwnerCall` arm for the case:

```
!import.namespace
&& import_source_matches_owner(...)
&& import.imported.as_deref() == Some(owner.name)
&& import.local != owner.name          // it's a rename
&& contains_call_name(body, import.local)
&& !local_identifier_declared_in_test_body(body, import.local)  // shadow guard
```

The existing namespace arm falls through to `ImportedOwnerCall` (unchanged).
A non-namespace import where `local == owner.name` is not a rename: it is handled
by the earlier `DirectOwnerCall` arm (no change).

### 3. Map `ImportAliasOwnerCall` → `DirectOwnerCall`/`High` in `ts_relation_to_domain`

```rust
TypeScriptRelationKind::ImportAliasOwnerCall => RelationReason::DirectOwnerCall,
// confidence:
TypeScriptRelationKind::ImportAliasOwnerCall => RelationConfidence::High,
```

No new `RelationReason` variant is needed — `DirectOwnerCall` is the correct
domain meaning: the test syntactically calls the owner.

## Shadow Guard

Because this upgrade moves the alias case to `High` confidence, we must close
the falsifier: a test that re-declares the alias name in the body —

```typescript
import { computeValue as cv } from '../src/compute';

test('shadow', () => {
    const cv = () => 42;   // shadows the import!
    expect(cv(5)).toBe(42);
});
```

— calls the LOCAL shadow, not the owner. The existing helper
`local_identifier_declared_in_test_body` (already used in
`module_initializer_observer_relation` and `class_method_owner_call_relation`)
is applied to `import.local` before crediting `ImportAliasOwnerCall`. If the
local is shadowed, the arm does NOT fire and the test falls through to whatever
the prior behavior was (likely uncredited / heuristic).

## Invariants

- Under-emit fix only: the alias-rename test was ALREADY credited (exposure class
  `exposed`); this spec only upgrades the stated reason and confidence.
- NEVER over-claim: wrong-name alias (`import { otherFn as cv }`, otherFn ≠ owner)
  must NOT be credited — `import.imported.as_deref() == Some(owner.name)` guard.
- NEVER credit a shadowed alias at High — shadow guard applied before crediting.
- Namespace imports (`import * as ns`): OUT OF SCOPE. They remain
  `ImportedOwnerCall` / `import_path_affinity` / Medium.

## Known limitation (named, not hidden)

The shadow guard `local_identifier_declared_in_test_body` is **line-based**: it
scans each source line for a `const`/`let`/`var` declaration of the alias at the
start of the (trimmed) line. A shadow declared **inline on the same source line**
as other statements — e.g. `test('x', () => { const cv = f; expect(cv(5))... })`
all on one physical line — is NOT detected, so such a case would still be credited
at `direct_owner_call` / High. This form is rare and discouraged by standard
formatters (prettier/eslint split it onto its own line, where the guard fires —
verified behaviorally). The mis-statement is confined to the **advisory relation
reason/confidence metadata**; it never changes the exposure class. This matches
the pre-existing behavior of the `ImportedOwnerCall` path, which used the same
line-based guard. A token-/AST-scoped shadow check is deferred (not in this slice).
- Default imports: OUT OF SCOPE. Remain on existing path.
- Re-export chains: OUT OF SCOPE. `ReExportChainFollowed` is unchanged.
- No `schema_version` bump. The `relation_reason: "direct_owner_call"` string is
  already a valid output-schema value.
- No version bump, no publish, no release workflow changes.
- Static-language clean: `ImportAliasOwnerCall` uses no forbidden terms.

## Required Evidence

Unit tests in
`crates/ripr/src/analysis/language/typescript/tests.rs`:

1. **POSITIVE** (`find_related_tests_matches_named_import_alias_calls` extended):
   `import { applyDiscount as subject }` + `subject(...)` → relation kind
   `ImportAliasOwnerCall`, relation_reason `direct_owner_call`,
   relation_confidence `high`.

2. **WRONG-NAME** (`find_related_tests_alias_wrong_name_not_credited`):
   `import { otherFn as cv }` (otherFn ≠ owner) + `cv(...)` →
   NOT `ImportAliasOwnerCall`.

3. **SHADOW** (`find_related_tests_alias_shadowed_local_not_credited_high`):
   `import { computeValue as cv }` + `const cv = ...; cv(5)` →
   NOT `ImportAliasOwnerCall` (shadow guard).

4. **NON-ALIAS UNCHANGED** (`find_related_tests_non_alias_import_still_direct_owner_call`):
   `import { computeValue }` + `computeValue(5)` → still `DirectOwnerCall`/high.

5. **NAMESPACE UNCHANGED** (`find_related_tests_namespace_import_unchanged_imported_owner_call`):
   `import * as ns` + `ns.computeValue(5)` → still `ImportedOwnerCall` →
   `import_path_affinity` / Medium.

## Test Mapping

| Test | Control case |
|---|---|
| `find_related_tests_matches_named_import_alias_calls` (extended) | 1 — POSITIVE: alias-rename + call → ImportAliasOwnerCall / direct_owner_call / High |
| `find_related_tests_alias_wrong_name_not_credited` | 2 — WRONG-NAME: alias of a different export is not credited |
| `find_related_tests_alias_shadowed_local_not_credited_high` | 3 — SHADOW: re-declared local binding not credited at High |
| `find_related_tests_non_alias_import_still_direct_owner_call` | 4 — NON-ALIAS: non-renaming import remains DirectOwnerCall |
| `find_related_tests_namespace_import_unchanged_imported_owner_call` | 5 — NAMESPACE: namespace import stays ImportedOwnerCall / import_path_affinity / Medium |

## Metrics

- `typescript_import_alias_owner_call_confidence_honesty`: presence of
  `relation_reason: "direct_owner_call"` and `relation_confidence: "high"` in
  alias-rename import findings for TypeScript preview. Verified by control 1
  (POSITIVE test). Exposure class is unchanged (`exposed` in both before and
  after); only the stated reason/confidence changes.

## Non-Goals

- Does NOT change the namespace import path (gap 2, separate PR).
- Does NOT change default import handling.
- Does NOT trace re-export chains through aliases.
- Does NOT change exposure class (`exposed` stays `exposed`).
- Does NOT add new `RelationReason` domain variants.
- Does NOT bump `schema_version`.
- Does NOT change language status (TypeScript stays Preview).
- Does NOT bump crate version or trigger a publish.

## Acceptance Examples

### Before (incorrect)

```json
{
  "relation_reason": "import_path_affinity",
  "relation_confidence": "medium"
}
```

### After (correct)

```json
{
  "relation_reason": "direct_owner_call",
  "relation_confidence": "high"
}
```

Exposure class unchanged: `exposed` in both cases.

## Implementation Mapping

| Behavior | Code location |
|---|---|
| `ImportAliasOwnerCall` variant, rank, uses_oracle, as_str | `crates/ripr/src/analysis/language/typescript/types.rs` |
| Alias-rename arm in `owner_call_relation` (with shadow guard) | `crates/ripr/src/analysis/language/typescript/related_tests.rs` |
| Domain mapping in `ts_relation_to_domain` | `crates/ripr/src/analysis/language/typescript/related_tests.rs` |
| Spec registration | `policy/doc-artifacts.toml`, `docs/specs/README.md` |
| Traceability | `.ripr/traceability.toml` |
