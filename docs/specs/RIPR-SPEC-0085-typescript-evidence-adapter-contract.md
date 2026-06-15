# RIPR-SPEC-0085: TypeScript Evidence Adapter Contract

Status: proposed

Owner: product / swarm

Created: 2026-06-12

Linked proposal:

- None yet

Linked ADRs:

- ADR 0008 — Rust-native `oxc` parser adoption for the TypeScript adapter

Linked plan:

- `docs/IMPLEMENTATION_CAMPAIGNS.md` — TypeScript evidence-adapter wave

Linked issues:

- #1150 — decompose the TypeScript adapter monolith (PR 0, prerequisite)
- #1146 — unify the per-surface fail-closed evidence-state vocabulary

Linked PRs:

- #1151 — behavior-preserving decomposition of `typescript.rs` into `analysis/language/typescript/` (the structural foundation this contract extends)

Support-tier impact:

- **No tier change. TypeScript stays `preview`.** This spec defines the
  contract by which the *existing* `oxc`-AST preview adapter becomes a
  reliable, bounded evidence adapter. It promotes nothing on its own.
  Promotion to a higher support tier requires dogfood evidence plus a
  TypeScript route-quality slice, and remains governed by the canonical
  ledger in [support tiers](../status/SUPPORT_TIERS.md).
- The contract is fail-closed by construction: an analysis either produces
  a *complete* bounded repair packet or a *named* limitation. Nothing in
  between is emitted as actionable.

Policy impact:

- Register this spec in `policy/doc-artifacts.toml`, the spec index
  (`docs/specs/README.md`), and `.ripr/traceability.toml`.
- No new crates, binaries, dependencies, parsers, runtime executors, or
  LSP servers introduced by this spec. The parser (`oxc`) is already
  adopted (ADR 0008).

## Problem

RIPR already ships a TypeScript adapter. It is **not** greenfield and it is
**not** a regex pile: it is built on the `oxc` AST (`oxc_parser` / `oxc_ast`
/ `oxc_span`, gated behind the `lang-typescript` feature, blessed by ADR
0008), and it already performs *preview-grade* test discovery, oracle
extraction (`toBe` / `toEqual` / `toThrow` / `rejects.toThrow`), owner and
related-test inference, a Bun cross-language bridge, static limits, and
actionability classification. Today every TypeScript finding is emitted as
`preview_advisory_only` with `repair_packet_ready: false` — that is, the
adapter is already fail-closed, but it cannot yet produce an *actionable*
result.

What is missing is not a parser and not a from-scratch analyzer. What is
missing is a **single contract** that says, for any TypeScript/JavaScript
test surface a diff touches:

> Produce exactly one of two things — a bounded repair packet that is safe
> to delegate, or a named limitation that explains precisely why it is not
> — and never anything in between.

Without that contract, the capability work (runner inference, package-root
discovery, the limitation taxonomy, the preview→actionable jump) has no
shared acceptance bar, and each surface is free to invent its own notion of
"actionable", which is how fake authority leaks in. This spec is the
umbrella contract the downstream capability PRs each implement a slice of.
It complements, and does not duplicate:

- [RIPR-SPEC-0027](RIPR-SPEC-0027-typescript-preview-static-facts.md) — the
  TypeScript-family static fact vocabulary (owner / test / assertion /
  probe facts).
- [RIPR-SPEC-0071](RIPR-SPEC-0071-typescript-bun-evidence-use-case.md) — the
  TypeScript/Bun cross-language evidence use case.
- [RIPR-SPEC-0062](RIPR-SPEC-0062-cross-language-oracle-graph.md) /
  [RIPR-SPEC-0063](RIPR-SPEC-0063-cross-language-evidence-router-ux.md) —
  the cross-language oracle graph and its surfacing.

## Behavior

### The two-outcome contract

For each changed TypeScript/JavaScript surface a diff reaches, the adapter
resolves to exactly one of:

1. **A bounded TypeScript repair packet** — emitted only when the full
   field contract below is satisfied. Safe for a human or coding agent to
   act on.
2. **A named TypeScript limitation** — emitted whenever any required field
   is unresolved. It routes the gap into analyzer backlog instead of
   silently dropping it or pretending it is actionable.

The trust rule is invariant across the wave:

```text
actionable  = the full repair-packet contract is satisfied
everything else = a named limitation, never a partial/implied packet
```

### Reality baseline (what already exists)

```text
parser:            oxc AST (ADR 0008), feature `lang-typescript`
discovery:         naming-pattern test-file detection (preview)
oracle extraction: expect toBe/toEqual/toStrictEqual/toThrow, rejects.toThrow (preview)
ownership:         owner + related-test inference (preview)
cross-language:    Bun bridge states (preview)
limits:            generic StaticLimitKind buckets
actionability:     always preview_advisory_only + repair_packet_ready=false
```

### Repair-packet field contract

A TypeScript repair packet is emitted **only when every field below is
present and evidence-backed**. A missing field is not a weaker packet — it
is a limitation.

```text
canonical_gap_id
language = "typescript"
gap_state = actionable
repair_kind
target                         # the test/observer file to edit
target_test_or_observer_shape  # e.g. expect(...).toThrow(...)
verify_command                 # evidence-backed; never invented (see runner rules)
receipt_command
allowed_edit_surface           # the edit cage
must_not_change                # e.g. native binding, public API
confidence
raw_evidence_refs              # file:line back to the AST evidence
```

Hard rules (no exceptions):

```text
no verify_command  -> no packet (limitation)
no target          -> no packet (limitation)
no edit cage       -> no packet (limitation)
no actionable state-> no packet (limitation)
never invent a verify_command from a file extension alone
never invent a target file
never suggest a Rust test for a TypeScript gap
```

### Supported runner inference

A `verify_command` is emitted only when repo evidence supports it
(package manifests, lockfiles, declared scripts, workspace layout):

```bash
bun test <file>
vitest run <file>
jest <file>
node --test <file>
npm test -- <file>
pnpm test -- <file>
yarn test <file>
```

An unresolved or ambiguous runner becomes the named limitation
`typescript_test_runner_unresolved`. No command is ever synthesized from
the file extension alone.

### Supported oracle shapes (v1)

```text
expect(value).toBe(...)            -> exact_value
expect(value).toEqual(...)         -> deep_value
expect(value).toStrictEqual(...)   -> deep_value
expect(fn).toThrow(...)            -> error_variant
await expect(p).rejects.toThrow(...) -> promise_rejection
assert.equal / strictEqual / deepStrictEqual -> exact_value / deep_value
t.equal / t.deepEqual              -> exact_value / deep_value
```

Each extracted oracle carries `file:line`, `oracle_kind`,
`observed_expression`, `expected_value_or_variant`, `confidence`, and a
`raw_evidence_ref`. A raw textual mention is never an oracle.

### TypeScript limitation taxonomy

Unresolved evidence routes to a *named* TypeScript limitation, each with a
sample source and a `repair_route` (analyzer backlog pointer):

```text
typescript_test_runner_unresolved
typescript_oracle_helper_gated
typescript_custom_matcher_unresolved
typescript_snapshot_discriminator_unresolved
typescript_table_case_unresolved
typescript_mock_only_observer
typescript_dynamic_assertion_unresolved
typescript_target_unresolved
typescript_import_graph_unresolved
```

### Source-to-test ownership and the cross-language boundary

Ownership prefers same-package, direct-import, nearby-naming, runner-path
evidence; unknown ownership fails closed into `typescript_target_unresolved`.

Cross-language Rust/native coverage rides on this adapter as one use case,
not the whole feature:

```text
Rust/native seam -> binding/export -> TS/JS callsite -> TS/JS oracle -> external evidence
```

Cross-language states stay honest: `typescript_oracle_established`,
`typescript_oracle_missing_discriminator`, `typescript_mention_not_observer`,
`bridge_unknown`, `cross_language_oracle_visibility_unresolved`. A mention is
not an observation; `bridge_unknown` is a *feature* (a named limitation),
not a guess.

## Required Evidence

- The `oxc`-parsed AST node (`Statement` / `CallExpression`) and its span
  for every extracted oracle (`raw_evidence_ref` = `file:line`).
- Repo manifest/lockfile/script evidence for any inferred `verify_command`.
- A named limitation, with sample source and `repair_route`, for every
  unresolved runner / oracle / ownership / bridge.
- Behavioral fixtures demonstrating that each unresolved shape produces a
  limitation and **not** a repair packet.

## Non-Goals

- Not full TypeScript type checking or semantic resolution.
- Not every test framework — only evidence-backed, supported runners.
- Not generated or synthesized tests.
- Not autonomous edits — RIPR inspects and routes; it does not edit code.
- Not a full native bridge graph — cross-language is bounded and fail-closed.
- Not a support-tier promotion — TypeScript stays `preview` until dogfood
  and route-quality justify otherwise.
- Not a re-selection of the parser — `oxc` is adopted (ADR 0008).
- Not a cosmetic rename of existing structs (`TypeScriptTest` etc. stay).
- Not a relocation of the adapter — it stays under
  `crates/ripr/src/analysis/language/typescript/`.

## Acceptance Examples

### A bounded TypeScript repair packet (all fields satisfied)

```yaml
canonical_gap_id: ts-error-variant-017
language: typescript
gap_state: actionable
repair_kind: add_error_variant_assertion
target: test/foo/bar.test.ts
target_test_or_observer_shape: expect(...).toThrow(...)
verify_command: bun test test/foo/bar.test.ts
receipt_command: ripr receipt write ...
allowed_edit_surface: [test/foo/bar.test.ts]
must_not_change: [src/native/binding.rs, public API]
confidence: high
raw_evidence_refs: [test/foo/bar.test.ts:42]
```

### A named TypeScript limitation (a required field is unresolved)

```yaml
gap_state: static_limitation
category: typescript_oracle_helper_gated
why_not_actionable: assertion is hidden behind an unresolved helper
repair_route: analysis/typescript-oracle-helper-resolution
sample_sources: [test/foo/bar.test.ts:42]
unlock_condition: resolve helper return shape or add supported oracle extraction
non_claims:
  - not a repair packet
  - do not invent verify command
  - do not suggest unrelated Rust tests
```

### Unknown runner fails closed

```text
input:  a test file with no resolvable runner (no lockfile/script evidence)
output: limitation typescript_test_runner_unresolved (NOT a packet, NOT an invented `bun test`)
```

## Test Mapping

This is a proposed contract spec; its acceptance is realized incrementally
by the downstream capability PRs, each of which adds its own fixtures and
tests under this contract. The current preview behavior the contract
extends is already covered by the adapter's in-tree tests
(`crates/ripr/src/analysis/language/typescript/tests.rs`) and the
TypeScript fixtures under `fixtures/`. No new tests are introduced by this
spec itself.

## Implementation Mapping

The adapter lives at `crates/ripr/src/analysis/language/typescript/`
(decomposed in #1151). This contract is implemented by the wave, in order:

```text
PR 1  (this spec)      adapter contract
PR 2  discovery.rs     package-root / monorepo discovery (enables verify + ownership)
PR 3  runner inference verify_command + typescript_test_runner_unresolved
PR 4  static_limit.rs  TypeScript limitation taxonomy
PR 5  oracle.rs        oracle-metadata hardening
PR 6  owners.rs / related_tests.rs  import + source-to-test ownership v1
PR 7  actionability.rs preview -> actionable repair-packet contract
PR 8  output/lsp/vscode surface projection (packets + limitations)
PR 9  bun_bridge.rs    cross-language bridge inventory (report-only)
PR 10 classifier.rs    cross-language oracle routing
PR 11 bun_bridge.rs    Bun stable-byte profile (consumes the adapter, never forks it)
PR 12 dogfood          real TS evidence-to-repair attempts
PR 13 reports          TypeScript route-quality slice
```

## Metrics

- `typescript_evidence_adapter_contract_status_proposed` — tracks the
  proposed status of this contract until the wave implements and dogfoods
  it. Real capability metrics (packets attempted/improved, unknown-runner
  count, helper-gated count, cross-language unknown-bridge count) are
  introduced by the route-quality PR and remain `not_available` until a
  real producer populates them.
