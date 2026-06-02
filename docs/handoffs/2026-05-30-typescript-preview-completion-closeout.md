# Closeout: TypeScript Preview Completion

Date: 2026-05-30

Owner: Lane 1 evidence-to-repair

Linked proposal: [RIPR-PROP-0001](../proposals/RIPR-PROP-0001-multi-language-adapter-preview.md)

Linked spec: [RIPR-SPEC-0027](../specs/RIPR-SPEC-0027-typescript-preview-static-facts.md)

Linked plan item: `campaign/typescript-preview-completion-closeout`

## Decision

TypeScript and JavaScript remain `preview`.

The completion lane proved that TypeScript-family preview evidence is useful as
an opt-in, advisory evidence-to-repair input. It did not prove that TS/JS
preview evidence should become a default gate input, public badge input,
baseline authority, RIPR Zero input, or calibrated-confidence support tier.

The next promotion decision must be a separate policy/support-tier packet. This
closeout deliberately does not promote TS/JS beyond preview.

## What landed

| Surface | Evidence |
| --- | --- |
| Syntax-first preview facts | TypeScript/JavaScript findings keep `language` and `language_status = preview`; fixture families cover owners, assertions/oracles, related tests, probe shapes, static limits, disabled config, parse errors, and mixed Rust/TypeScript output. |
| Strict preview actionability | Preview findings carry `gap_state`, `actionability_category`, `why_not_actionable`, `repair_route`, missing actionability fields, and raw preview evidence refs instead of emitting public repair packets. |
| LSP preview projection | Diagnostic data, hover, and inspect-context actions carry the same preview actionability context without repair-packet, verify, receipt, edit, or generated-test actions for incomplete preview evidence. |
| Generated CI grouping | Generated CI can group TypeScript and JavaScript advisory evidence separately, with actionability summaries, repair-packet-ready counts, static-limit context, and `gate_impact = none`. |
| Repair-loop dogfood receipts | `fixtures/typescript-preview-repair-loop/corpus.json` records TS/JS advisory receipts for boundary proof, weak-oracle downgrades, async broad-error evidence, skipped incomplete-packet routes, mocked-module static limitations, and unchanged already-observed evidence. |
| Route-quality metrics | Attempt-ledger and readiness reports emit `language_repair_route_quality[]` so TS/JS preview outcomes are measured by language and repair kind without becoming public packets, badge inputs, or gates. |

## Proof executed

The final route-quality metric PR passed:

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

Closeout validation for this PR is docs/control-plane validation:

```bash
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-pr
git diff --check
```

## Claim and support-tier changes

Support tier remains `preview`.

Users may rely on TypeScript/JavaScript preview evidence for opt-in advisory
evaluation, static-limit inspection, LSP context, generated-CI grouping, and
route-quality learning. Users must not treat it as Rust parity or as an
actionable repair queue.

## Policy ledger updates

No gate, badge, baseline, RIPR Zero, branch-protection, or public support-tier
policy changes are made by this closeout.

## Remaining work

- A later promotion packet may evaluate a narrow `usable alpha` claim if TS/JS
  preview evidence can produce bounded public repair packets with target shape,
  verify command, receipt command, allowed edit surface, must-not-change
  constraints, confidence basis, and raw evidence refs.
- Runtime TypeScript/Jest/Vitest execution, package graph resolution,
  typechecker integration, provider calls, generated tests, autonomous edits,
  and mutation execution remain out of scope.
- Static limitations such as mocked modules, dynamic dispatch, missing import
  graph, metaprogramming, decorator indirection, unsupported syntax, ambiguous
  const expressions, and computed-member calls remain named limitations or
  advisory evidence.

## Archive updates

- `docs/lanes/LANE_1_TYPESCRIPT_PREVIEW_COMPLETION.md` records this lane as
  closed with TS/JS remaining preview.
- `plans/typescript-preview-completion/implementation-plan.md` marks
  `campaign/typescript-preview-completion-closeout` done.
- `docs/status/SUPPORT_TIERS.md`, `metrics/capabilities.toml`,
  `docs/CAPABILITY_MATRIX.md`, and `.ripr/traceability.toml` point to this
  closeout as the current proof boundary.
