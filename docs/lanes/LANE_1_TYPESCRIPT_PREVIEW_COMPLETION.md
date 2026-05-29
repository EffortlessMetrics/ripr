# Lane 1: TypeScript Preview Completion

Status: PR 0 audit reconciled; fixture harness next

Date: 2026-05-29

Scope: PR 0 audit only. This tracker records what already exists, what is only
specified, and what remains missing before TypeScript/JavaScript preview can be
treated as a trusted evidence-to-repair surface. It does not change analyzer
behavior, active goals, support tiers, gates, badges, or release authority.

## Product Boundary

TypeScript and JavaScript are not a blank slate. They already exist as opt-in
preview evidence through the language adapter work. The useful lane is not Rust
parity. The useful lane is:

```text
TS/JS source signal
-> canonical evidence item
-> strict actionability decision
-> bounded repair packet or named limitation
-> verify command
-> receipt command
-> advisory projection through CLI, CI, LSP, and agent surfaces
```

The lane must preserve these boundaries:

- TypeScript/JavaScript stay opt-in and advisory until an explicit promotion
  packet says otherwise.
- Preview evidence does not participate in default gates, public badges,
  baseline debt, RIPR Zero, branch protection, or calibrated-confidence
  authority.
- The adapter remains syntax-first: no `tsc`, `tsserver`, package graph,
  Jest/Vitest runtime execution, bundlers, sourcemaps, mutation execution,
  provider calls, generated tests, or source edits.
- Source `EffortlessMetrics/ripr` remains release and distribution authority;
  this tracker belongs to `ripr-swarm` development work.

## Current Audit

Current docs already claim a completed Language Adapter Preview campaign:

- `docs/status/SUPPORT_TIERS.md` lists TypeScript and JavaScript as `preview`.
- `docs/LANGUAGE_ADAPTER_PREVIEW.md` documents opt-in preview adapters,
  preview labels, static-limit interpretation, editor routing, generated CI
  grouping, and rollback.
- `docs/handoffs/2026-05-13-campaign-27-closeout.md` says Campaign 27 shipped
  TypeScript/JavaScript preview facts, editor projection, generated CI grouping,
  workflow docs, and dogfood receipts.
- `.ripr/goals/archive/2026-05-13-language-adapter-preview.toml` records the
  closed campaign and the TypeScript preview work item.

The detailed archive is narrower than the headline docs. It records a first
useful TypeScript preview loop, while explicitly deferring structured polish
such as additional owner kinds, richer static limits, and full projection into
strict repair packets.

Live GitHub checks on 2026-05-29 found no open TypeScript/JavaScript,
language-adapter, or RIPR-SPEC-0027 PR or issue carrying this work in either
`EffortlessMetrics/ripr-swarm` or source `EffortlessMetrics/ripr`.

## Implemented Today

Router and config:

- `.ts`, `.tsx`, `.js`, and `.jsx` route to the TypeScript-family adapter.
- `[languages] enabled = ["rust"]` remains the default.
- Adding `typescript` enables the preview adapter when the binary has the
  `lang-typescript` feature.
- The default Cargo feature set includes `lang-typescript`.
- Config validation rejects unknown languages, duplicates, and unavailable
  preview features.
- `ripr doctor` reports enabled languages.

Adapter facts:

- Diff-mode TypeScript findings are produced for changed production lines that
  fall inside recognised owners.
- Owners currently cover top-level function declarations and named exported
  function declarations.
- Test files are detected by `.test` and `.spec` suffixes for `.ts`, `.tsx`,
  `.js`, and `.jsx`.
- Test extraction covers top-level `test(...)` and `it(...)` calls.
- Assertion extraction covers exact-value matchers, broad `toThrow`, async
  `resolves`/`rejects` chains, snapshot matchers, mock expectations, smoke
  matchers, and relational matchers.
- Related-test matching is token-aware enough to avoid comments, strings,
  block comments, and arbitrary object-method calls for function owners.
- Probe-family classification distinguishes predicate, return value, error
  path, field construction, and side-effect call line shapes.
- Structured `static_limit_kind = "mocked_module"` is emitted when related
  tests use syntactic `vi.mock(...)` or `jest.mock(...)`.

Projection and proof:

- Human and JSON check output carry `language = "typescript"` or
  `language = "javascript"` and `language_status = "preview"` for
  TypeScript-family findings.
- LSP diagnostics preserve preview metadata and `static_limit_kind` when
  present.
- Generated CI has language grouping support for configured preview languages.
- `cargo xtask dogfood` has a TypeScript preview receipt for
  `mocked_module`, disabled-language behavior, preview labels, and no
  cross-language related-test routing.
- Existing TypeScript fixture families cover boundary gap, strong oracle,
  return-value shape, owner-file matching, broad `toThrow`, awaited rejected
  promise, effect probes, and mocked-module static limit.

## Missing Slices

These are the missing slices that should drive the next PRs. Keep each slice
PR-sized and do not promote support tier while they land.

Completed after the initial audit:

- `RIPR-SPEC-0026` and `RIPR-SPEC-0027` are accepted and aligned with the
  Campaign 27 boundary.
- `plans/typescript-preview-completion/implementation-plan.md` sequences the
  remaining completion work without changing analyzer behavior or support-tier
  status.
- `.js` and `.jsx` findings emitted by the TypeScript-family adapter are now
  separately labeled `language = "javascript"`.
- The current `check` parser still has no `--languages rust,typescript`
  override. Config remains the supported opt-in surface for this lane; CLI
  override support is deferred to a later app/config contract change.

1. Fixture harness completion
   - Current TypeScript fixture directories use `.ts` inputs only.
   - Missing fixture families: `.tsx`, `.js`, `.jsx`, parse error /
     `unsupported_syntax`, mixed Rust + TypeScript repo, TypeScript disabled,
     and TypeScript enabled.
   - Next step: add harness fixtures before broadening adapter behavior.

2. Owner facts
   - Current owner facts do not populate `owner_kind` for TypeScript findings.
   - Missing owner shapes: arrow functions assigned to `const`/`let`, class
     methods, default exports, React-ish function components, and module-level
     const initializers.
   - Next step: emit owner kind metadata and fixture each supported owner kind.

3. Test and assertion facts
   - Current extraction handles common matcher chains, but only inside
     top-level `test`/`it` bodies.
   - Missing or under-fixtured shapes: nested `describe`, `test.each`,
     `it.each`, table-driven tests, snapshot downgrade fixtures, smoke-only
     downgrade fixtures, and top-level expect handling when safe.
   - Next step: extend fixture-backed test and assertion facts without
     turning weak or snapshot evidence into strong proof.

4. Related-test matching
   - Current related-test matching is direct owner-call text matching with
     important negative guards.
   - Missing heuristics: imported owner reference, same-file proximity,
     `describe` block naming, and test-name token matching.
   - Next step: add positive and negative fixtures for each heuristic and make
     ambiguous matches limitations, not actionable recommendations.

5. Probe facts and discriminator candidates
   - Current probe classification is line-shape based and does not attach
     candidate values.
   - Missing probe quality: safe predicate boundary candidates, object/field
     construction detail, mock interaction detail, and richer source spans.
   - Next step: keep ambiguous line shapes out of actionable repair packets
     until the adapter can name the target shape safely.

6. Static-limit taxonomy
   - Current structured TypeScript static limit is `mocked_module`.
   - Missing limit kinds: `dynamic_dispatch`, `metaprogramming`,
     `missing_import_graph`, `unsupported_syntax`, and decorator indirection
     when decorators are encountered.
   - Next step: emit named limitations with human reason and repair route
     instead of silently dropping parse or unsupported syntax cases.

7. Repo-mode and output projection
   - TypeScript `analyze_repo` currently returns no findings.
   - Check output carries preview metadata, but current TypeScript findings do
     not yet become complete canonical repair packets with strict
     actionability fields.
   - Next step: project TS/JS preview metadata through repo exposure, agent,
     first-pr/pilot, PR summary, and any SARIF-supported path without a schema
     fork.

8. Strict TypeScript actionability
   - Current TypeScript findings carry `recommended_next_step`, but not a
     complete repair packet.
    - Missing required actionability fields: `canonical_gap_id`, `gap_state`,
      `repair_kind`, `target_test_or_observer_shape`, `verify_command`,
      `receipt_command`, `confidence`, `evidence_refs`, and
      `must_not_change`.
    - Next step: no TypeScript item should become actionable unless every
      required field is present; otherwise emit a named limitation or
      `missing_context` route.

9. LSP / VS Code repair packet UX
   - Current LSP projection carries preview metadata and static limits.
    - Missing TypeScript proof: enabled/disabled TS e2e with a complete repair
      packet, hover boundary, code action packet, verify command, receipt
      command, and constraints.
    - Next step: keep editor actions projection-only and suppress repair code
      actions unless the packet is complete.

10. Generated CI grouping proof
   - Generated CI grouping exists for configured preview languages.
    - Missing TypeScript completion proof: grouping over real TS/JS repair
      packets and limitations while preserving advisory-only gate impact.
    - Next step: keep TS/JS preview evidence out of default blocking, badges,
      baselines, and RIPR Zero.

11. Dogfood, route-quality metrics, and support-tier decision
   - Current dogfood covers TypeScript mocked-module preview and a small
     projection boundary.
    - Missing proof: real TS/JS repair-loop receipts, weak-oracle downgrades,
      limitation examples, false-actionable review, and route-quality metrics
      by repair kind and language.
    - Next step: only after those receipts and metrics should support tiers be
      reviewed. The default decision remains `preview`.

## PR Sequence

Use the user-provided PR sequence as the campaign outline, with this audit as
PR 0. The first two follow-up slices have already landed on `main`:

```text
PR 1: spec(ts): accept TypeScript preview static-facts contract
PR 2: analysis(ts): route TypeScript preview files through language adapter
```

PR 2 landed the core route/config behavior and separate JavaScript preview
labels. It deliberately deferred a `--languages` CLI override because the
current `check` parser has no such option.

The next safe PR is:

```text
PR 3: analysis(ts): add TypeScript preview fixture harness
```

## Validation

Docs-only tracker changes should run:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
git diff --check
```

The next behavior-changing TypeScript PR should add the narrowest relevant
subset of:

```bash
cargo test -p ripr language_adapter --lib
cargo test -p ripr typescript --lib
cargo test -p ripr lsp --lib
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
```
