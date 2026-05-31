# TypeScript Preview Completion Plan

Status: closed lane plan; not a support-tier promotion
Owner: language-adapter-swarm
Plan artifact: RIPR-PLAN-0027
Linked proposal: RIPR-PROP-0001
Linked specs: RIPR-SPEC-0026, RIPR-SPEC-0027, RIPR-SPEC-0061
Linked ADRs: ADR-0008
Active goal: n/a; work proceeds only by explicit PR selection

## Current State

TypeScript and JavaScript are opt-in preview evidence surfaces. Campaign 27
landed the language adapter boundary, first useful TypeScript-family preview
facts, editor projection, generated-CI grouping, and dogfood receipts. The
remaining work is not Rust parity; it is a bounded evidence-to-repair loop that
can emit a complete repair packet or a named limitation without overclaiming.

This plan keeps the preview boundary intact:

- no `tsc`, `tsserver`, package graph, Jest/Vitest execution, bundler,
  sourcemap, provider, generated-test, source-edit, or mutation execution
  dependency;
- no default CI blocking, public badge contribution, baseline debt, RIPR Zero
  input, or support-tier promotion;
- no Rust behavior change except fixture-backed regression protection.

## Work Item: spec/typescript-preview-contract-reconciliation

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: ADR-0008
Blocks: analysis/typescript-router-config-conformance
Blocked by: n/a

### Goal

Accept and narrow the TypeScript-family preview static-facts contract so later
implementation agents can tell current implementation, accepted target, and
deferred proof apart.

### Production Delta

Docs/spec only. No analyzer behavior, output producer, gate, badge, editor,
generated-CI, provider, runtime, source-edit, or mutation behavior changes.

### Acceptance

- RIPR-SPEC-0026 and RIPR-SPEC-0027 no longer conflict with the Campaign 27
  closeout status.
- RIPR-SPEC-0027 names JavaScript as a separately labeled preview surface that
  uses the TypeScript-family adapter implementation.
- The TypeScript-family fact vocabulary names the bounded `language`,
  `language_status`, `owner_kind`, `test_kind`, `assertion_kind`, `probe_kind`,
  `static_limit_kind`, and `repair_kind` fields.
- Traceability, capability, plan, and lane docs point to this implementation
  queue.
- No support-tier promotion.

### Proof Commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
git diff --check
```

### Rollback

Revert the docs-only reconciliation commit. No generated artifacts or runtime
state need rollback.

## Work Item: analysis/typescript-router-config-conformance

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0026, RIPR-SPEC-0027
Linked ADR: ADR-0008
Blocks: fixtures/typescript-preview-harness
Blocked by: spec/typescript-preview-contract-reconciliation

### Goal

Make `.ts`, `.tsx`, `.js`, and `.jsx` routing, opt-in config, JavaScript
labeling, and adapter-unavailable fail-closed behavior boring and fixture-backed.

### Acceptance

- TypeScript-family files are ignored unless `typescript` is enabled.
- TypeScript files are labeled `typescript`; JavaScript files are labeled
  `javascript`.
- Disabling TypeScript removes TS/JS diagnostics, packets, CI grouping, and LSP
  findings.
- CLI override support is explicitly deferred because current `check` parsing
  has no `--languages` option; repo config remains the supported opt-in surface
  for this slice.

### Proof Commands

```bash
cargo test -p ripr language_adapter --lib
cargo test -p ripr typescript --lib
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

## Work Item: fixtures/typescript-preview-harness

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: ADR-0008
Blocks: analysis/typescript-owner-facts
Blocked by: analysis/typescript-router-config-conformance

### Goal

Create deterministic fixture families for `.ts`, `.tsx`, `.js`, `.jsx`, parse
errors, unsupported syntax, mixed Rust plus TypeScript repos, enabled preview,
and disabled preview before broadening behavior.

### Acceptance

- Parse success and parse failure paths are fixture-backed.
- Unsupported syntax emits `static_limit_kind = "unsupported_syntax"` where the
  public field exists.
- No incomplete TypeScript repair packet is projected as actionable.

### Proof Commands

```bash
cargo test -p ripr typescript --lib
cargo xtask fixtures typescript_tsx_preview
cargo xtask fixtures javascript_js_preview
cargo xtask fixtures javascript_jsx_preview
cargo xtask fixtures typescript_disabled
cargo xtask fixtures typescript_parse_error_unsupported_syntax
cargo xtask fixtures mixed_rust_typescript_preview
cargo xtask check-traceability
cargo xtask check-capabilities
git diff --check
```

## Work Item: analysis/typescript-owner-facts

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: ADR-0008
Blocks: analysis/typescript-test-assertion-facts
Blocked by: fixtures/typescript-preview-harness

### Goal

Emit owner facts and `owner_kind` for function declarations, arrow consts,
methods, exported/default forms, obvious React-ish components, and module
initializers without matching comments or strings.

### Acceptance

- TypeScript/JavaScript findings project structural `probe.owner` metadata and
  additive `owner_kind` without forking schemas.
- Owner extraction recognises function declarations, exported functions, arrow
  consts, instance methods, static class methods, default exported functions
  and class methods, obvious TSX/JSX components, and module-level
  initializers.
- Comment and string contents do not create synthetic owners.
- Method and module-initializer findings keep no-path guidance bounded to the
  current missing related-test context instead of claiming safe call guidance.
- No support-tier promotion, default gate, badge, baseline, RIPR Zero, runtime,
  provider, generated-test, source-edit, or Rust behavior change.

### Proof Commands

```bash
cargo test -p ripr typescript --lib
cargo xtask fixtures typescript_owner_kinds
cargo xtask fixtures
cargo xtask goldens check
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-static-language
git diff --check
```

## Work Item: analysis/typescript-test-assertion-facts

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: ADR-0008
Blocks: analysis/typescript-related-test-matching
Blocked by: analysis/typescript-owner-facts

### Goal

Fixture-backed Jest/Vitest `describe`, `test`, `it`, `.each`, exact-value,
error-path, async resolve/reject, mock interaction, snapshot, and smoke oracle
facts, with weak oracles kept weak.

### Acceptance

- Test discovery recurses through nested `describe(...)` bodies.
- Array-form `test.each(...)` and `it.each(...)` calls are discovered.
- Exact-value matchers, async `resolves`, mock interaction assertions,
  snapshot assertions, and smoke-only assertions are fixture-backed.
- Snapshot, mock, and smoke evidence stays weak/advisory instead of becoming
  strong TypeScript repair proof.
- No support-tier promotion, default gate, badge, baseline, RIPR Zero, runtime
  test execution, provider call, generated-test, source-edit, or Rust behavior
  change.

### Proof Commands

```bash
cargo test -p ripr typescript --lib
cargo xtask fixtures typescript_jest_vitest_assertion_facts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-static-language
git diff --check
```

## Work Item: analysis/typescript-related-test-matching

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: ADR-0008
Blocks: analysis/typescript-probe-facts
Blocked by: analysis/typescript-test-assertion-facts

### Goal

Add token-aware direct calls, imported owner references, same-file proximity,
describe naming, and test-name token matching with negative fixtures for
strings, comments, unrelated method calls, unrelated property names, and mocked
module indirection.

### Current Delta

Named import alias calls and namespace import member calls are related only when
the relative import source resolves to the changed owner file. Unrelated import
sources, type-only imports, arbitrary object method calls, comments, and string
mentions stay non-related.

Same-stem file proximity, `describe(...)` owner-name proximity, and test-name
owner-token proximity are related as uncertainty-only links. They do not borrow
strong assertions as proof, and partial owner-token names remain non-related.

### Remaining Delta

No remaining PR 6 related-test matching slice. Method receiver relation,
module-initializer relation, and complete repair-packet actionability remain
deferred to later work items.

## Work Item: analysis/typescript-probe-facts

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: ADR-0008
Blocks: analysis/typescript-static-limit-taxonomy
Blocked by: analysis/typescript-related-test-matching

### Goal

Emit predicate, return-value, error-path, field/object construction, call
side-effect, and mock-interaction probes with source spans, owner linkage,
candidate values only when safe, and explicit confidence.

### Current Delta

TypeScript/JavaScript preview findings now carry probe expectations and
required-oracle templates for specific line shapes. Weak findings with trusted
related-test evidence receive flow sinks where syntax supports one and
missing-discriminator candidates for predicate boundaries, return values,
thrown/rejected errors, field/object construction, call side effects,
mock interactions, and log/output text. Ambiguous const expressions and
computed-member calls keep advisory preview output without invented
discriminator guidance.

### Proof Commands

```bash
cargo test -p ripr typescript --lib
cargo xtask fixtures typescript_probe_facts
cargo xtask goldens check
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

## Work Item: analysis/typescript-static-limit-taxonomy

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: ADR-0008
Blocks: analysis/typescript-strict-actionability
Blocked by: analysis/typescript-probe-facts

### Goal

Surface `dynamic_dispatch`, `metaprogramming`, `missing_import_graph`,
`mocked_module`, `unsupported_syntax`, and `decorator_indirection` as named
limitations with human reasons and repair routes, not actionable packets.

### Current Delta

The TypeScript-family preview classifier now emits structured static-limit
metadata for computed member calls, metaprogramming syntax, decorated owners,
production calls through imported symbols, related mocked modules, and
parser-error unsupported syntax. Static limits are exposed as evidence and
missing context only; TypeScript/JavaScript remain opt-in advisory preview.

### Proof Commands

```bash
cargo test -p ripr typescript --lib
cargo xtask fixtures typescript_static_limit_taxonomy
cargo xtask fixtures typescript_mocked_module_limit
cargo xtask fixtures typescript_parse_error_unsupported_syntax
cargo xtask goldens check
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-static-language
git diff --check
```

## Work Item: output/typescript-preview-metadata

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0026, RIPR-SPEC-0027
Linked ADR: n/a
Blocks: lsp/typescript-preview-repair-context
Blocked by: none

### Goal

Project TypeScript/JavaScript preview metadata through human, JSON,
repo-exposure, agent, first-pr/pilot, PR summary, and SARIF-supported surfaces
without a schema fork or Rust behavior change.

### Current Delta

TypeScript/JavaScript preview findings now project fail-closed actionability
metadata as structured `preview_actionability` in the current TypeScript-bearing
surfaces: check JSON and diff-scoped SARIF properties. Human output shows a
dedicated Preview actionability section, and GitHub annotations preserve the
advisory preview/no-packet boundary. The projection carries `gap_state`,
`actionability_category`, `why_not_actionable`, `repair_route`, missing
actionability fields, evidence needed to promote, parsed raw evidence refs,
`authority_boundary = "preview_advisory_only"`, and
`repair_packet_ready = false`.

Repo-exposure, agent packet, first-pr/pilot repair packet, PR summary, gate,
badge, baseline, and RIPR Zero surfaces are intentionally unchanged here because
TypeScript still has no complete repair packets or GapRecords. Those surfaces
continue to fail closed until the later repair-packet/LSP/CI slices explicitly
add bounded TS packet eligibility.

### Proof Commands

```bash
cargo test -p ripr output::preview_actionability --lib
cargo test -p ripr finding_json_projects_typescript_preview_actionability --lib
cargo test -p ripr render_finding_includes_preview_actionability_without_raw_string_spam --lib
cargo test -p ripr sarif_preserves_preview_actionability_properties --lib
cargo test -p ripr render_includes_preview_actionability_boundary --lib
cargo xtask fixtures typescript_strict_actionability
cargo xtask fixtures typescript_probe_facts
cargo xtask fixtures typescript_static_limit_taxonomy
cargo xtask fixtures typescript_parse_error_unsupported_syntax
cargo xtask fixtures typescript_mocked_module_limit
cargo xtask goldens check
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-static-language
git diff --check
```

## Work Item: analysis/typescript-strict-actionability

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027, RIPR-SPEC-0061
Linked ADR: n/a
Blocks: output/typescript-preview-metadata
Blocked by: analysis/typescript-static-limit-taxonomy

### Goal

Make a TS/JS gap actionable only when it has `canonical_gap_id`, `gap_state`,
`repair_kind`, target test/observer shape, verify command, receipt command,
confidence, evidence refs, and `must_not_change`; otherwise emit a named
limitation, advisory, or missing-context route.

### Current Delta

TypeScript/JavaScript preview findings now fail closed with explicit
`gap_state`, `actionability_category`, `why_not_actionable`, `repair_route`,
missing actionability fields, evidence needed to promote, and raw preview
evidence refs in the existing finding evidence. Static limits become
`static_limitation`, strong exact-oracle evidence becomes `already_observed`,
and incomplete weak candidates remain advisory instead of repair-packet
eligible.

### Proof Commands

```bash
cargo test -p ripr typescript --lib
cargo xtask fixtures typescript_strict_actionability
cargo xtask fixtures typescript_static_limit_taxonomy
cargo xtask goldens check
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-static-language
git diff --check
```

## Work Item: lsp/typescript-preview-repair-context

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: ADR-0011
Blocks: ci/typescript-preview-language-grouping-proof
Blocked by: none

### Goal

Project TypeScript/JavaScript preview repair context in VS Code only when the
preview language is enabled and the repair packet is complete.

### Current Delta

TypeScript/JavaScript preview findings now carry structured
`preview_actionability` into LSP diagnostic data. Finding hover renders the
preview actionability state before RIPR evidence details, and the inspect
context code action copies the same preview context for agent handoff.

Incomplete preview findings remain bounded to inspect and refresh actions: no
repair packet, verify, receipt, edit, or generated-test action is exposed
without complete actionability.

### Proof Commands

```bash
cargo fmt
cargo test -p ripr lsp --lib
```

## Work Item: ci/typescript-preview-language-grouping-proof

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027, RIPR-SPEC-0038
Linked ADR: n/a
Blocks: dogfood/typescript-preview-repair-loop
Blocked by: none

### Goal

Prove generated-CI TypeScript/JavaScript grouping over preview repair packets
and limitations while preserving advisory-only default behavior.

### Current Delta

Generated GitHub CI now expands configured TypeScript preview grouping to the
TypeScript-family evidence labels that can appear in artifacts: `typescript`
and separately labeled `javascript`. Each preview-language group reports
artifact entries, preview entries, missing preview-status count, classifications,
static-limit counts/kinds, actionability state/category counts,
repair-packet-ready count, and an explicit `gate_impact = none` boundary. The
grouping remains hidden for Rust-only configuration and remains advisory
presentation only.

### Proof Commands

```bash
cargo fmt
cargo test -p ripr init_generated_github_workflow_groups_preview_languages_only_when_configured --lib
cargo test -p xtask dogfood_generated_ci_cockpit_receipts_are_checked --bin xtask
cargo test -p xtask dogfood_language_preview_scenarios_cover_projection_boundaries --bin xtask
cargo test -p xtask dogfood_language_preview_run_checks_static_limit_receipt --bin xtask
```

## Work Item: dogfood/typescript-preview-repair-loop

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: n/a
Blocks: metrics/typescript-preview-route-quality
Blocked by: none

### Goal

Record real TS/JS repair-loop receipts for at least one improved/resolved case,
one limitation, one weak-oracle downgrade, and unchanged or skipped cases.

### Result

`fixtures/real-repair-attempts/corpus.json` records TypeScript and JavaScript
preview dogfood receipts for LSP preview context, generated-CI preview
grouping, mocked-module static-limitation routing, and weak-oracle
non-promotion. Those receipts remain repair-loop evidence only: they do not make
preview evidence public repair packets, swarm-ready work, badge inputs, or CI
gates.

`fixtures/typescript-preview-repair-loop/corpus.json` records TypeScript and
JavaScript preview repair-loop receipts for boundary predicate advisory proof,
smoke and snapshot weak-oracle downgrades, async broad-error evidence, a
JavaScript mock-interaction skipped route, a mocked-module static limitation,
and an already-observed JavaScript unchanged case. `cargo xtask dogfood`
projects those receipts while preserving `repair_packet_ready = false`,
`preview_advisory_only`, no runtime Jest/Vitest execution, no source edits, no
generated tests, and no gate, badge, baseline, RIPR Zero, or support-tier
promotion authority.

### Proof Commands

```bash
cargo test -p xtask dogfood_typescript_preview_repair_loop --bin xtask
cargo test -p xtask dogfood_real_repair_attempt_receipts_are_checked -- --test-threads=1
cargo xtask dogfood
cargo xtask ripr-swarm attempt-ledger
cargo xtask ripr-swarm readiness
cargo xtask check-fixture-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

## Work Item: metrics/typescript-preview-route-quality

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: n/a
Blocks: campaign/typescript-preview-completion-closeout
Blocked by: none

### Goal

Track attempted, improved, unchanged, regressed, resolved, success rate,
missing evidence fields, and failing routes by language and repair kind without
automatic promotion.

### Current Delta

Attempt-ledger and readiness reports now preserve `language` on attempt rows and
emit `language_repair_route_quality[]`, an additive projection grouped by
language and repair kind. TypeScript/JavaScript preview receipt outcomes can be
measured separately from Rust repair-route quality without becoming public
repair packets, badge inputs, blocking gates, or support-tier evidence by
itself.

### Proof Commands

```bash
cargo test -p xtask ripr_swarm_attempt_ledger_imports_real_repair_attempts -- --test-threads=1
cargo test -p xtask ripr_swarm_readiness_recomputes_route_quality_from_latest_attempts_when_rows_absent -- --test-threads=1
cargo xtask ripr-swarm attempt-ledger
cargo xtask ripr-swarm readiness
cargo xtask check-output-contracts
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-static-language
cargo xtask check-pr
git diff --check
```

## Work Item: campaign/typescript-preview-completion-closeout

Status: done
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: n/a
Blocks: n/a
Blocked by: none

### Goal

Decide whether TypeScript/JavaScript stay `preview` or earn a narrow
`usable alpha` support claim, with proof links and deferred work explicit.

### Current Delta

Decision: TypeScript and JavaScript stay `preview`. The completion lane proved
opt-in advisory static evidence, strict preview actionability, LSP/generated-CI
projection, repair-loop dogfood receipts, and route-quality metrics by language
and repair kind. It did not prove public repair-packet eligibility, default gate
or badge authority, baseline/RIPR Zero participation, runtime Jest/Vitest
execution, generated tests, autonomous edits, provider work, or mutation proof.

### Proof Commands

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```
