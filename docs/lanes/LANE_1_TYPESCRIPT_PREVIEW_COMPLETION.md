# Lane 1: TypeScript Preview Completion

Status: Closed; TypeScript and JavaScript remain opt-in preview

Date: 2026-05-29

Scope: living Lane 1 tracker. This tracker records what already exists, what is
only specified, and what remains missing before TypeScript/JavaScript preview
can be treated as a trusted evidence-to-repair surface. It does not by itself
change support tiers, gates, badges, or release authority.

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
- Related-test matching also recognises same-stem file proximity,
  `describe(...)` owner-name proximity, and test-name owner-token proximity as
  explicit uncertain links. Those links do not borrow assertion strength or
  become complete repair guidance.
- Probe facts distinguish predicate, return value, error path, field/object
  construction, side-effect calls, mock interactions, and log/output text
  through the existing side-effect family.
- Weak trusted related-test findings now carry expected sink/oracle templates,
  flow sinks where syntax supports one, and missing-discriminator candidates
  only when the changed expression shape is specific enough.
- Ambiguous const-expression and computed-member-call shapes stay advisory and
  do not invent missing-discriminator guidance.
- Structured `static_limit_kind = "mocked_module"` is emitted when related
  tests use syntactic `vi.mock(...)` or `jest.mock(...)`.

Projection and proof:

- Human and JSON check output carry `language = "typescript"` or
  `language = "javascript"` and `language_status = "preview"` for
  TypeScript-family findings.
- Check JSON and diff-scoped SARIF now carry structured
  `preview_actionability` for TypeScript/JavaScript preview findings. Human
  output shows the same state in a dedicated Preview actionability section, and
  GitHub annotations preserve the advisory preview/no-packet boundary.
- LSP diagnostics preserve preview metadata, `static_limit_kind`, and structured
  `preview_actionability` when present.
- LSP hover renders the preview actionability state before evidence details,
  and inspect-context actions copy the same preview context without exposing a
  repair packet, verify, receipt, edit, or generated-test action for incomplete
  preview findings.
- Generated CI has language grouping support for configured preview languages,
  including separate TypeScript-family `typescript` and `javascript` groups
  when TypeScript preview is configured.
- Generated CI preview groups summarize actionability states/categories,
  repair-packet-ready counts, static-limit context, and explicit
  `gate_impact = none` advisory boundaries.
- `cargo xtask dogfood` has a TypeScript preview receipt for
  `mocked_module`, disabled-language behavior, preview labels, and no
  cross-language related-test routing.
- Existing TypeScript fixture families cover boundary gap, strong oracle,
  return-value shape, owner-file matching, broad `toThrow`, awaited rejected
  promise, effect probes, mocked-module static limit, nested `describe`,
  `test.each`, `it.each`, snapshot downgrade, smoke downgrade, mock
  interaction, async `resolves` evidence, imported owner calls,
  heuristic-only name/proximity links, and probe facts for predicate,
  return-value, error-path, field/object, side-effect, mock-interaction, and
  log/output shapes.

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
- Same-stem file proximity, `describe(...)` owner-name proximity, and test-name
  owner-token proximity are fixture-backed by
  `fixtures/typescript_related_test_name_proximity` as uncertain links that
  stay weak/advisory and do not use assertions as proof. Partial owner tokens
  remain non-related.
- Probe facts are fixture-backed by `fixtures/typescript_probe_facts` for
  `.ts`, `.tsx`, `.js`, and `.jsx`, including missing-discriminator candidates
  for specific weak findings and no invented discriminator for ambiguous const
  expressions or computed-member calls.

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
   - Status: done for the PR 6 scope.
   - Current related-test matching covers direct owner-call text matching,
     relative named import alias and namespace import owner calls, same-stem
     file proximity, `describe(...)` owner-name proximity, and test-name
     owner-token proximity.
   - Negative guards keep unrelated imports, type-only imports, arbitrary object
     methods, strings, comments, and partial owner-token names from becoming
     trusted related-test evidence.
   - Heuristic-only links are explicit uncertainty evidence and do not use
     extracted assertions as proof.

5. Probe facts and discriminator candidates
   - Status: done for the PR 7 scope.
   - Current probe facts attach owner, source span, family, changed expression,
     expected sinks/oracles, optional flow sink, and safe
     missing-discriminator candidates.
   - Predicate, return-value, error-path, field/object construction,
     side-effect call, mock-interaction, and log/output shapes are covered.
   - Ambiguous line shapes remain advisory and do not receive discriminator
     candidates.

6. Static-limit taxonomy
   - Status: done for the PR 8 scope.
   - Structured TypeScript/JavaScript preview static limits now cover
     `dynamic_dispatch`, `metaprogramming`, `missing_import_graph`,
     `decorator_indirection`, `mocked_module`, and parser-error
     `unsupported_syntax`.
   - The new fixture family `typescript_static_limit_taxonomy` covers
     dynamic dispatch, metaprogramming, decorator indirection, and missing
     import graph across TypeScript-family sources. Existing fixtures keep
     `mocked_module` and parser-error `unsupported_syntax` pinned.
   - Static limitations are emitted as evidence and missing context only; they
     do not promote TypeScript/JavaScript beyond advisory preview.

7. Strict TypeScript actionability
   - Status: done for the fail-closed PR 9 scope.
   - TypeScript/JavaScript preview findings now emit explicit
     `gap_state`, `actionability_category`, `why_not_actionable`,
     `repair_route`, missing actionability fields, evidence needed to promote,
     and raw preview evidence refs in the existing evidence stream.
   - Strong related Jest/Vitest evidence is marked `already_observed`, static
     limits are marked `static_limitation`, no-path and heuristic-only
     relations stay advisory, and weak direct findings stay
     `incomplete_repair_packet` until canonical repair-packet fields exist.
   - The fixture family `typescript_strict_actionability` pins advisory
     incomplete-packet, already-observed, and missing-context states.
   - No TypeScript repair packet, default gate, badge, baseline, or RIPR Zero
     authority is emitted by this slice.

8. Repo-mode and output projection
   - Status: done for the current TS-bearing output surfaces.
   - Check JSON and diff-scoped SARIF now carry structured
     `preview_actionability` with gap state, category, why-not-actionable,
     repair route, missing fields, promotion evidence needs, raw evidence refs,
     `authority_boundary = "preview_advisory_only"`, and
     `repair_packet_ready = false`.
   - Human output renders a Preview actionability section instead of relying on
     raw actionability strings in the Evidence and Weakness sections.
   - GitHub annotations include the advisory preview/no-packet boundary.
   - Repo exposure, agent packet, first-pr/pilot repair packet, PR summary,
     gate, badge, baseline, and RIPR Zero surfaces remain unchanged because
     TypeScript still has no complete repair packets or GapRecords.

9. LSP / VS Code repair packet UX
   - Status: done for the current incomplete-packet preview context.
   - Current LSP projection carries preview metadata, static limits, and
     structured `preview_actionability` in diagnostic data.
   - Hover shows the preview actionability state before RIPR evidence, and the
     inspect-context action copies the same preview context for agent handoff.
   - Incomplete TypeScript/JavaScript preview findings stay bounded to inspect
     and refresh actions. Repair-packet, verify, receipt, edit, and
     generated-test actions remain suppressed until a later complete packet
     exists.
   - Remaining TypeScript proof: enabled/disabled TS e2e with a complete repair
     packet once strict packet eligibility exists.

10. Generated CI grouping proof
   - Status: done for the PR 12 scope.
   - Generated CI grouping remains opt-in by configured preview languages and
     hidden for Rust-only configuration.
   - TypeScript preview configuration expands the TypeScript-family grouping to
     separately labeled `typescript` and `javascript` evidence, because
     JavaScript findings remain JavaScript preview findings.
   - Each group reports actionability state/category counts, repair-packet-ready
     counts, static-limit context, and explicit `gate_impact = none`.
   - TS/JS preview evidence remains out of default blocking, badges, baselines,
     and RIPR Zero.

11. Dogfood, route-quality metrics, and support-tier decision
   - Current dogfood covers TypeScript mocked-module preview, generated-CI
     TypeScript-family language grouping, real TS/JS repair-loop receipts,
     weak-oracle downgrade/non-promotion, limitation examples, unchanged
     already-observed evidence, intentionally skipped incomplete-packet cases,
     and route-quality metrics by language and repair kind while preserving
     `repair_packet_ready = false` and advisory preview authority.
   - Closeout decision: TypeScript and JavaScript remain `preview`; a later
     promotion would need a separate support-tier packet with complete repair
     packets and route-quality proof.

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

The PR 6 related-test matching slices have landed:

```text
PR 6a: analysis(ts): relate imported TypeScript owner calls
PR 6b: analysis(ts): relate TypeScript tests by name and proximity signals
```

PR 6a recognises named import aliases and namespace imports when the relative
source maps to the changed owner file, while keeping unrelated imports,
type-only imports, arbitrary object methods, strings, and comments out of
related-test evidence. PR 6b adds same-stem file, `describe(...)` name, and
test-name token proximity as uncertainty-only related-test locations.

PR 7 landed TypeScript preview probe facts without support-tier promotion,
runtime execution, generated tests, source edits, default gates, badge
contribution, baseline authority, or RIPR Zero contribution.

PR 8 landed TypeScript preview static-limit taxonomy for dynamic dispatch,
metaprogramming, missing import graph, decorator indirection, mocked modules,
and parser-error unsupported syntax without support-tier promotion, runtime
execution, generated tests, source edits, default gates, badge contribution,
baseline authority, or RIPR Zero contribution.

PR 9 landed fail-closed TypeScript strict actionability, PR 10 landed the
current output metadata projection, and PR 11 landed LSP preview actionability
context without noisy diagnostics or repair-packet actions for incomplete
preview findings. PR 12 landed generated-CI TypeScript-family language grouping
with separate TypeScript and JavaScript preview labels, actionability summaries,
and `gate_impact = none` while preserving advisory-only default behavior.

PR 13 landed TypeScript-family repair-loop dogfood receipts for boundary
predicate advisory proof, smoke and snapshot weak-oracle downgrades, async
broad-error evidence, a JavaScript mock-interaction skipped route, mocked-module
static limitation, and already-observed JavaScript unchanged evidence without
support-tier promotion, complete repair-packet claims, runtime Jest/Vitest
execution, generated tests, source edits, default gates, badge contribution,
baseline authority, or RIPR Zero contribution.

The TypeScript preview completion lane is closed. Future work should be a
separate support-tier promotion or analyzer-improvement packet, not a
continuation of this completion lane.

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

## Latest dogfood receipt proof

The TypeScript preview repair-loop dogfood slice now records four real receipt cases in `fixtures/real-repair-attempts/corpus.json`:

- `typescript_preview_lsp_repair_context_improved`: advisory LSP context became inspectable without becoming a public repair packet.
- `javascript_preview_generated_ci_grouping_improved`: generated CI grouping is receipted as advisory preview evidence with no gate or badge claim.
- `typescript_preview_mocked_module_limitation_improved`: mocked-module evidence remains a static limitation with analyzer-backlog value.
- `typescript_preview_weak_oracle_downgrade_unchanged`: weak-oracle preview evidence intentionally remains advisory and unchanged.

These receipts are dogfood evidence for the repair loop only. They do not promote TypeScript or JavaScript preview evidence to public repair packets, swarm-ready work, blocking CI gates, or badge inputs.

## Latest route-quality metric proof

Attempt-ledger and readiness reports now emit `language_repair_route_quality[]`
from latest attempts that carry a known `language`. The projection is grouped by
language and repair kind, so TypeScript and JavaScript preview outcomes can be
measured without treating preview evidence as public repair packets, swarm-ready
work, badge input, or blocking CI authority.

Closeout decision: TypeScript and JavaScript remain `preview`. The current
evidence proves opt-in advisory projection, repair-loop receipts, and
route-quality learning, but not a public repair-packet queue, default gate
input, badge input, baseline authority, RIPR Zero input, or
calibrated-confidence support-tier move. See
[TypeScript Preview Completion closeout](../handoffs/2026-05-30-typescript-preview-completion-closeout.md).
