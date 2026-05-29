# TypeScript Preview Completion Plan

Status: proposed lane plan; not a support-tier promotion
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

Status: active
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

Status: ready
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
- CLI override support is either implemented with tests or explicitly deferred
  with a named limitation.

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

Status: ready
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

## Work Item: analysis/typescript-owner-facts

Status: ready
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: ADR-0008
Blocks: analysis/typescript-test-assertion-facts
Blocked by: fixtures/typescript-preview-harness

### Goal

Emit owner facts and `owner_kind` for function declarations, arrow consts,
methods, exported/default forms, obvious React-ish components, and module
initializers without matching comments or strings.

## Work Item: analysis/typescript-test-assertion-facts

Status: ready
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: ADR-0008
Blocks: analysis/typescript-related-test-matching
Blocked by: analysis/typescript-owner-facts

### Goal

Fixture-backed Jest/Vitest `describe`, `test`, `it`, `.each`, exact-value,
error-path, async resolve/reject, mock interaction, snapshot, and smoke oracle
facts, with weak oracles kept weak.

## Work Item: analysis/typescript-related-test-matching

Status: ready
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

## Work Item: analysis/typescript-probe-facts

Status: ready
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: ADR-0008
Blocks: analysis/typescript-static-limit-taxonomy
Blocked by: analysis/typescript-related-test-matching

### Goal

Emit predicate, return-value, error-path, field/object construction, call
side-effect, and mock-interaction probes with source spans, owner linkage,
candidate values only when safe, and explicit confidence.

## Work Item: analysis/typescript-static-limit-taxonomy

Status: ready
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: ADR-0008
Blocks: output/typescript-preview-metadata
Blocked by: analysis/typescript-probe-facts

### Goal

Surface `dynamic_dispatch`, `metaprogramming`, `missing_import_graph`,
`mocked_module`, `unsupported_syntax`, and `decorator_indirection` as named
limitations with human reasons and repair routes, not actionable packets.

## Work Item: output/typescript-preview-metadata

Status: ready
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0026, RIPR-SPEC-0027
Linked ADR: n/a
Blocks: analysis/typescript-strict-actionability
Blocked by: analysis/typescript-static-limit-taxonomy

### Goal

Project TypeScript/JavaScript preview metadata through human, JSON,
repo-exposure, agent, first-pr/pilot, PR summary, and SARIF-supported surfaces
without a schema fork or Rust behavior change.

## Work Item: analysis/typescript-strict-actionability

Status: ready
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027, RIPR-SPEC-0061
Linked ADR: n/a
Blocks: lsp/typescript-preview-repair-context
Blocked by: output/typescript-preview-metadata

### Goal

Make a TS/JS gap actionable only when it has `canonical_gap_id`, `gap_state`,
`repair_kind`, target test/observer shape, verify command, receipt command,
confidence, evidence refs, and `must_not_change`; otherwise emit a named
limitation, advisory, or missing-context route.

## Work Item: lsp/typescript-preview-repair-context

Status: ready
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: ADR-0011
Blocks: ci/typescript-preview-language-grouping-proof
Blocked by: analysis/typescript-strict-actionability

### Goal

Project TypeScript/JavaScript preview repair context in VS Code only when the
preview language is enabled and the repair packet is complete.

## Work Item: ci/typescript-preview-language-grouping-proof

Status: ready
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: n/a
Blocks: dogfood/typescript-preview-repair-loop
Blocked by: lsp/typescript-preview-repair-context

### Goal

Prove generated-CI TypeScript/JavaScript grouping over preview repair packets
and limitations while preserving advisory-only default behavior.

## Work Item: dogfood/typescript-preview-repair-loop

Status: ready
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: n/a
Blocks: metrics/typescript-preview-route-quality
Blocked by: ci/typescript-preview-language-grouping-proof

### Goal

Record real TS/JS repair-loop receipts for at least one improved/resolved case,
one limitation, one weak-oracle downgrade, and unchanged or skipped cases.

## Work Item: metrics/typescript-preview-route-quality

Status: ready
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: n/a
Blocks: campaign/typescript-preview-completion-closeout
Blocked by: dogfood/typescript-preview-repair-loop

### Goal

Track attempted, improved, unchanged, regressed, resolved, success rate,
missing evidence fields, and failing routes by language and repair kind without
automatic promotion.

## Work Item: campaign/typescript-preview-completion-closeout

Status: ready
Linked proposal: RIPR-PROP-0001
Linked spec: RIPR-SPEC-0027
Linked ADR: n/a
Blocks: n/a
Blocked by: metrics/typescript-preview-route-quality

### Goal

Decide whether TypeScript/JavaScript stay `preview` or earn a narrow
`usable alpha` support claim, with proof links and deferred work explicit.
