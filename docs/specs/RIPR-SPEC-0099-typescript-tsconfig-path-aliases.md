# RIPR-SPEC-0099: TypeScript tsconfig.json Path-Alias Resolution

Status: accepted

Owner: product / swarm

Created: 2026-06-14

Linked issues:

- None

Linked PRs:

- None yet

Support-tier impact:

- Opt-in improvement for TypeScript preview analysis: when
  `[typescript] resolve_tsconfig_paths = true` is set in `.ripr.toml`,
  `@/owner`-style aliased imports can now be credited as owner↔test links
  instead of silently producing `no_static_path`. Language status stays
  Preview; no tier change.
  Claim boundaries and tier labels remain governed by the canonical ledger in
  [support tiers](../status/SUPPORT_TIERS.md).

Policy impact:

- No new crates, binaries, or LSP servers.
- New module: `crates/ripr/src/analysis/language/typescript/tsconfig.rs`.
- New config key: `[typescript] resolve_tsconfig_paths` (default `false`).
- No schema version bump. `typescript_path_alias_unresolved` is additive
  evidence on existing findings; it does not change the JSON schema version.
- Register this spec in `policy/doc-artifacts.toml`.

## Problem

The TypeScript adapter resolves import specifiers relative-only.
`normalized_relative_import_module` (in `related_tests.rs`) returns `None`
for any specifier that does not start with `./` or `../`. Consequently,
a test that imports the owner via a tsconfig `paths` alias (e.g.
`import { applyDiscount } from '@/owner'`) is NEVER credited as an
owner↔test link — the finding stays `no_static_path` with an empty
`related_tests` list. This is a false result: the test may have a strong
`toBe` oracle that would otherwise flip the finding to `exposed`.

Compound problem: the false `no_static_path` is SILENTLY dishonest. It
reads as "you have no tests," giving the user no hint that the gap is an
alias-resolution artifact rather than an actual coverage hole.

## Solution

### 1. Config opt-in (default OFF)

Add `[typescript] resolve_tsconfig_paths = false` to the ripr config model.
The feature is opt-in because tsconfig path resolution without running the
TypeScript compiler is inherently approximative and honesty-risky. Default
OFF preserves the existing conservative behavior.

### 2. Single-hop alias loader (`tsconfig.rs`)

`load_alias_map(root: &Path) -> Option<TsAliasMap>` reads
`root/tsconfig.json` then `root/jsconfig.json`. It is **fail-closed** on
every ambiguity:

- Missing file, JSON parse error, or missing `compilerOptions.baseUrl` → `None`.
- `extends` or `references` present → `None` (no transitive following).
- `paths` value array with length > 1 → entry excluded (multi-entry
  fail-closed).
- Template contains more than one `*` → entry excluded.

`TsAliasMap::resolve(specifier) -> Option<PathBuf>` returns a workspace-
relative path ONLY when ALL of:

1. Specifier is non-relative (does not start with `./` or `../`).
2. A literal or single-`*` glob key matches.
3. The matched value array has exactly one entry.
4. The value template has at most one `*`.
5. After substituting the captured `*`, the candidate path resolves to
   EXACTLY ONE existing workspace file (`.ts`/`.tsx`/`.js`/`.jsx`).
   Zero or >1 matches → `None`.

### 3. Resolver threading

`Option<&TsAliasMap>` is threaded through:
- `normalized_relative_import_module` — for non-relative specifiers, consults
  alias map before returning `None`; on success substitutes the resolved
  workspace-relative path into the existing relative-resolution pipeline.
- `ReExportIndex::build` and `ReExportIndex::resolve_to_owner` — so re-export
  chains also benefit from alias resolution.
- All downstream callers: `import_source_matches_owner`,
  `owner_name_shadowed_by_unrelated_import`, `owner_call_relation`,
  `find_related_tests`, `related_test_candidates`, `classify_change`.
- Alias map is built once at the `analyze_diff` call site and passed as
  `Option<&TsAliasMap>` (None when flag OFF).

### 4. Always-on honesty disclosure

Regardless of the `resolve_tsconfig_paths` flag,
`named_limitations_for_alias_unresolved` fires whenever a test has a
non-relative, name-matched import that was NOT credited as an owner relation.
The limitation `typescript_path_alias_unresolved` is emitted as additive
evidence on the finding and is CLASSIFICATION-NEUTRAL (does not flip
`no_static_path` to `exposed`). It explains to the user that an aliased
import plausibly targets the owner but could not be resolved.

Scope: name-matched non-relative imports only. Third-party imports (e.g.
`lodash`, `react`) whose imported symbol name does NOT match the owner's
exported name do NOT trigger the disclosure.

## Acceptance Criteria

### AC-1 (POSITIVE, flag ON)

Given:
- `tsconfig.json` with `baseUrl="."` and `paths={"@/*":["src/*"]}`.
- `src/owner.ts` exists on disk.
- A test at `src/owner.test.ts` that imports `applyDiscount` from `@/owner`
  with a strong `toBe(90)` assertion.
- `resolve_tsconfig_paths = true`.

When `classify_change` is called for a changed line in `src/owner.ts`:

Then: `finding.class == Exposed`, `related_tests.len() == 1`,
NO `typescript_path_alias_unresolved` limitation in evidence.

### AC-2 (DEFAULT-OFF CONTROL)

Identical setup, `resolve_tsconfig_paths` omitted (default false):

Then: `finding.class == NoStaticPath`, `related_tests.is_empty()`,
AND `typescript_limitation: typescript_path_alias_unresolved` IS present
in `finding.evidence`.

### AC-3 (AMBIGUOUS FAIL-CLOSED, flag ON)

Given:
- `tsconfig.json` with `paths={"@/*":["src/*","lib/*"]}` (multi-entry value).
- `src/owner.ts` exists.
- Test imports owner from `@/owner`.
- `resolve_tsconfig_paths = true`.

Then: alias map excludes the multi-entry key → resolution returns `None` →
`finding.class == NoStaticPath`, AND
`typescript_limitation: typescript_path_alias_unresolved` IS emitted.

### AC-4 (NON-MATCH NEGATIVE)

Given:
- Test imports `cloneDeep` from `lodash` (non-relative, does NOT match
  owner name `applyDiscount`).
- No `resolve_tsconfig_paths` flag.

Then: NO `typescript_path_alias_unresolved` limitation is emitted.

## Behavior

### Alias resolution scope

Resolution is single-hop and file-only: `load_alias_map` reads only
`compilerOptions.baseUrl` and `compilerOptions.paths` from the first of
`tsconfig.json` / `jsconfig.json` found at the workspace root. It does NOT
follow `extends` or `references`.

### Resolution decision tree

```
specifier starts with "./" or "../"  →  existing relative resolver (no change)
alias_map is None (flag OFF)         →  None (fail-closed, no guessing)
alias_map is Some:
  specifier matches a literal key    →  try unique_file_for(template)
  specifier matches a glob key       →  expand template, try unique_file_for
  no key matches                     →  None
unique_file_for:
  0 files found                      →  None
  1 file found                       →  Some(workspace-relative path)
  2+ files found                     →  None (ambiguous, fail-closed)
```

### Disclosure limitation scope

`typescript_path_alias_unresolved` fires on the FIRST uncredited test
that has at least one non-relative import whose `imported` symbol name
matches the owner's exported name. At most ONE disclosure per finding.
It does NOT fire for:
- Third-party imports (`lodash`, `react`) where the imported name does not
  match the owner.
- Namespace imports (`import * as X from '...'`) which do not pinpoint a
  single exported name.
- Tests that were already credited as owner↔test links.

## Required Evidence

Unit tests in `crates/ripr/src/analysis/language/typescript/tests.rs`:

1. `tsconfig_alias_resolution_flag_on_credits_test_as_exposed` — POSITIVE
   (AC-1): tsconfig with single-`*` alias, unique file on disk, flag ON →
   `class: exposed`, 1 related test, NO alias limitation.

2. `tsconfig_alias_resolution_flag_off_stays_no_static_path_with_disclosure`
   — DEFAULT-OFF CONTROL (AC-2): identical setup, no alias map → `class:
   no_static_path`, `typescript_path_alias_unresolved` present in evidence.

3. `tsconfig_alias_resolution_multi_entry_value_fails_closed` — AMBIGUOUS
   FAIL-CLOSED (AC-3): multi-entry value array, flag ON → alias excluded
   from map → `class: no_static_path`, disclosure present.

4. `tsconfig_alias_non_owner_import_emits_no_limitation` — NON-MATCH
   NEGATIVE (AC-4): third-party import `lodash/cloneDeep`, name mismatch →
   NO `typescript_path_alias_unresolved` emitted.

## Non-Goals

- Following `extends` or `references` chains — single-hop only.
- Full TypeScript project-graph or per-package tsconfig resolution.
- Node.js module resolution (bare specifier without paths alias).
- Dynamic or computed paths keys.
- `baseUrl`-only resolution without a `paths` entry (would require full node
  resolution semantics).
- Bumping the schema version for this additive evidence field.

## Files Changed

| File | Change |
|------|--------|
| `crates/ripr/src/config.rs` | `RawTypescriptConfig`; `typescript` field in `RawConfig` |
| `crates/ripr/src/config/model.rs` | `TypescriptConfig` struct + accessor |
| `crates/ripr/src/analysis/mod.rs` | `resolve_tsconfig_paths: bool` field in `AnalysisOptions` |
| `crates/ripr/src/app/check/options_builder.rs` | Wire config → `AnalysisOptions` |
| `crates/ripr/src/analysis/language/typescript/tsconfig.rs` | NEW: alias loader |
| `crates/ripr/src/analysis/language/typescript/related_tests.rs` | Thread `Option<&TsAliasMap>` |
| `crates/ripr/src/analysis/language/typescript/classifier.rs` | Thread alias map; collect alias limitations |
| `crates/ripr/src/analysis/language/typescript/static_limit.rs` | `named_limitations_for_alias_unresolved` |
| `crates/ripr/src/analysis/language/typescript/mod.rs` | Build alias map; register tsconfig module |
| `crates/ripr/src/analysis/language/typescript/tests.rs` | AC-1 through AC-4 tests |

## Acceptance Examples

### Before (alias import silently dropped)

```
tsconfig:  {"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}
test:      import { applyDiscount } from '@/owner';
           expect(applyDiscount(100, 10)).toBe(90);
result:    class: no_static_path  ← alias not resolved, test dropped silently
```

### After (flag ON, alias resolved)

```
resolve_tsconfig_paths = true
result:    class: exposed
           related_tests: ["applyDiscount returns correct value"]
           NO typescript_path_alias_unresolved limitation
```

### After (flag OFF, disclosure emitted)

```
resolve_tsconfig_paths = false  (default)
result:    class: no_static_path  (unchanged)
           evidence includes: typescript_limitation: typescript_path_alias_unresolved
```

### Control (third-party import — no disclosure)

```
test:      import { cloneDeep } from 'lodash';  // "cloneDeep" != owner "applyDiscount"
result:    NO typescript_path_alias_unresolved limitation emitted
```

## Test Mapping

- `crates/ripr/src/analysis/language/typescript/tests.rs::tests::tsconfig_alias_resolution_flag_on_credits_test_as_exposed`
- `crates/ripr/src/analysis/language/typescript/tests.rs::tests::tsconfig_alias_resolution_flag_off_stays_no_static_path_with_disclosure`
- `crates/ripr/src/analysis/language/typescript/tests.rs::tests::tsconfig_alias_resolution_multi_entry_value_fails_closed`
- `crates/ripr/src/analysis/language/typescript/tests.rs::tests::tsconfig_alias_non_owner_import_emits_no_limitation`

## Implementation Mapping

- `crates/ripr/src/analysis/language/typescript/tsconfig.rs` — `TsAliasMap`, `load_alias_map`, `parse_alias_map`, `GlobEntry`, `TsAliasMap::resolve`, `TsAliasMap::unique_file_for`
- `crates/ripr/src/analysis/language/typescript/related_tests.rs` — `normalized_relative_import_module` (non-relative arm), all downstream callers threaded with `alias_map`
- `crates/ripr/src/analysis/language/typescript/static_limit.rs` — `named_limitations_for_alias_unresolved`
- `crates/ripr/src/analysis/language/typescript/classifier.rs` — `classify_change` alias limitation collection; `#[allow(clippy::too_many_arguments)]`
- `crates/ripr/src/analysis/language/typescript/mod.rs` — alias map construction in `analyze_diff`
- `crates/ripr/src/config.rs` + `crates/ripr/src/config/model.rs` — `RawTypescriptConfig`, `TypescriptConfig`
- `crates/ripr/src/analysis/mod.rs` — `resolve_tsconfig_paths: bool` in `AnalysisOptions`

## Metrics

- `tsconfig_alias_flag_on_exposed` — flag ON + unique resolution → `exposed` (AC-1)
- `tsconfig_alias_flag_off_disclosure` — flag OFF → `no_static_path` + disclosure limitation (AC-2)
- `tsconfig_alias_multi_entry_fail_closed` — multi-entry value → fail-closed + disclosure (AC-3)
- `tsconfig_alias_non_owner_no_disclosure` — third-party import → no disclosure (AC-4)
