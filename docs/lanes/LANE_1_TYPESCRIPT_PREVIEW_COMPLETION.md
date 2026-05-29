# Lane 1: TypeScript Preview Completion

Status: imported related-test matching landed; name/proximity heuristics next

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
- Owners currently cover function declarations, exported/default functions,
  arrow functions assigned to `const`/`let`, instance methods, static class
  methods, obvious TSX/JSX components, and module-level const initializers.
- TypeScript/JavaScript findings project structural `probe.owner` metadata and
  `owner_kind` through human and JSON output.
- Test files are detected by `.test` and `.spec` suffixes for `.ts`, `.tsx`,
  `.js`, and `.jsx`.
- Test extraction covers top-level and nested `test(...)` / `it(...)` calls,
  including array-form `test.each(...)` and `it.each(...)`.
- Assertion extraction covers exact-value matchers, broad `toThrow`, async
  `resolves`/`rejects` chains, snapshot matchers, mock expectations, smoke
  matchers, and relational matchers.
- Related-test matching is token-aware enough to avoid comments, strings,
  block comments, and arbitrary object-method calls for function owners.
- Related-test matching recognises named import aliases and namespace import
  member calls only when the relative import source resolves to the changed
  owner file. Type-only imports and unrelated import sources remain
  non-related.
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
  promise, effect probes, mocked-module static limit, nested `describe`,
  `test.each`, `it.each`, snapshot downgrade, smoke downgrade, mock
  interaction, and async `resolves` evidence.

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
- The fixture harness now covers `.tsx`, `.js`, `.jsx`, mixed Rust plus
  TypeScript, Rust-only disabled TypeScript, and parser-error
  `unsupported_syntax` preview limitations.
- Owner facts now project structural owner ids and `owner_kind` for functions,
  arrow functions, methods, class methods, TSX/JSX components, and module
  initializers.
- Test and assertion facts now cover nested `describe` discovery,
  `test.each`, `it.each`, exact-value assertions, async `resolves`, mock
  interaction assertions, snapshots, and smoke-only assertions, with snapshot
  and smoke evidence staying weak.
- The current `check` parser still has no `--languages rust,typescript`
  override. Config remains the supported opt-in surface for this lane; CLI
  override support is deferred to a later app/config contract change.
- Named import alias and namespace import related-test matching is now
  fixture-backed by `fixtures/typescript_related_test_matching`, including
  false-match guards for unrelated imports, type-only imports, arbitrary object
  methods, strings, and comments.

1. Fixture harness completion
   - Status: done.
   - Fixture families now cover `.ts`, `.tsx`, `.js`, `.jsx`, parse error /
     `unsupported_syntax`, mixed Rust + TypeScript repo, TypeScript disabled,
     and TypeScript enabled.
   - Next step: keep new behavior slices fixture-first and avoid projecting
     incomplete TypeScript repair packets as actionable.

2. Owner facts
   - Status: done.
   - TypeScript/JavaScript findings now populate structural `probe.owner`
     metadata and `owner_kind`.
   - Fixture and unit coverage includes function declarations, exported/default
     functions, arrow consts, instance methods, static class methods, obvious
     TSX/JSX components, and module-level const initializers.
   - Method and module-initializer no-path guidance stays bounded to missing
     related-test context instead of claiming safe call guidance.

3. Test and assertion facts
   - Status: done.
   - Extraction now handles nested `describe`, top-level `test`/`it`, and
     array-form `test.each` / `it.each` bodies.
   - Fixture coverage pins exact-value, async `resolves`, mock interaction,
     snapshot, and smoke-only oracles.
   - Snapshot, mock, and smoke evidence stays advisory/weak unless a later
     strict repair packet can name a safe target shape.
   - Top-level expect handling remains deferred until the adapter can avoid
     unsafe file-level assertion association.

4. Related-test matching
   - Current related-test matching covers direct owner-call text matching plus
     relative named import alias and namespace import owner calls with
     important negative guards.
   - Missing heuristics: same-file proximity, `describe` block naming, and
     test-name token matching.
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
PR 0. The completed follow-up slices have landed on `main`:

```text
PR 1: spec(ts): accept TypeScript preview static-facts contract
PR 2: analysis(ts): route TypeScript preview files through language adapter
PR 3: analysis(ts): add TypeScript preview fixture harness
PR 4: analysis(ts): emit TypeScript owner facts
PR 5: analysis(ts): emit Jest and Vitest assertion facts
```

PR 2 landed the core route/config behavior and separate JavaScript preview
labels. It deliberately deferred a `--languages` CLI override because the
current `check` parser has no such option. PR 4 landed structural owner ids and
`owner_kind` projection without changing support-tier, gate, badge, baseline,
RIPR Zero, runtime, provider, generated-test, or source-edit behavior. PR 5
landed nested Jest/Vitest test discovery plus table-test and weak-oracle
fixture coverage without promoting TypeScript evidence.

The first PR 6 sub-slice landed import-owner related-test matching:

```text
PR 6a: analysis(ts): relate imported TypeScript owner calls
```

It recognises named import aliases and namespace imports when the relative
source maps to the changed owner file, while keeping unrelated imports,
type-only imports, arbitrary object methods, strings, and comments out of
related-test evidence.

The next safe PR is:

```text
PR 6b: analysis(ts): relate TypeScript tests by name and proximity signals
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
