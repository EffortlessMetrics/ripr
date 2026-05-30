# Support Tiers

This page answers the adoption question before the implementation question:

```text
Can I use this today, where, and what should I not over-trust yet?
```

The deeper source of truth remains the [capability matrix](../CAPABILITY_MATRIX.md)
and its machine-readable source in
[`metrics/capabilities.toml`](../../metrics/capabilities.toml). This page is
the buyer-readable map over those artifacts.

## Tier Vocabulary

| Tier | Meaning |
| --- | --- |
| `stable building block` | Fixture-backed behavior that is expected to hold inside its documented static-analysis scope. |
| `usable` | End-to-end product loop is proof-linked and ready for advisory adoption in its documented scope, but is not a stability or runtime-adequacy claim. |
| `usable alpha` | Implemented and useful for advisory PR work, with known static limits and proof artifacts. |
| `preview` | Opt-in surface that is fixture-backed enough to evaluate, but not a default promise or gate input. |
| `scaffold` | Plumbing exists, but the product loop is not useful yet without the next fact-extraction slices. |
| `blocked` | The surface is intentionally waiting on a named upstream capability. |
| `deferred` | Valid product direction, but not part of the current safe adoption path. |

All tiers are static evidence tiers. None of them means runtime mutation
adequacy, coverage adequacy, or general correctness.

## Current Support Map

| Capability | Tier | Surface | Proof | Known limits |
| --- | --- | --- | --- | --- |
| Rust static exposure loop | `usable alpha` | CLI, generated CI, editor, reports | [RIPR-SPEC-0001](../specs/RIPR-SPEC-0001-static-exposure-loop.md), [capability matrix](../CAPABILITY_MATRIX.md), `cargo xtask fixtures`, `cargo xtask goldens check` | Static only; unknowns stay explicit; mutation testing remains the runtime backstop. |
| Rust gap repair loop | `usable` | CLI, generated CI, PR repair cards, editor packets, agent packets, receipts | [First successful PR workflow](../FIRST_PR_WORKFLOW.md), [gap decision ledger spec](../specs/RIPR-SPEC-0046-gap-decision-ledger.md), `cargo xtask check-output-contracts`, `cargo xtask check-capabilities` | Advisory static loop only; interruptions require a repair route and verification command; runtime mutation and coverage remain separate signals. |
| Local delta flow and activation/value modeling | `stable building block` | Rust analysis output and evidence records | [capability matrix](../CAPABILITY_MATRIX.md#capability-matrix), [Lane 1 tracker](../lanes/LANE_1_EVIDENCE_SPINE.md), `cargo xtask lane1-evidence-audit` | Stable inside documented syntax-first scope; unsupported propagation and value sources remain static limitations. |
| First useful PR action | `usable alpha` | Generated CI summary, reports, editor projection | [First useful action workflow](../FIRST_USEFUL_ACTION_WORKFLOW.md), [RIPR-SPEC-0020](../specs/RIPR-SPEC-0020-first-useful-action-report.md), `cargo xtask check-output-contracts` | Advisory routing only; missing or stale inputs must be refreshed before assigning work. |
| PR review cockpit | `usable alpha` | Generated CI summary and uploaded report packet | [PR review front panel workflow](../PR_REVIEW_FRONT_PANEL_WORKFLOW.md), [Report packet index workflow](../REPORT_PACKET_INDEX_WORKFLOW.md), `cargo xtask check-output-contracts` | Composes explicit artifacts; summaries do not create analyzer truth or pass/fail authority. |
| Agent repair packets | `usable alpha` | CLI, editor handoff, reports | [Quickstart agent path](../QUICKSTART.md#agent-or-reviewer-first-hour), [agent workflows](../AGENT_WORKFLOWS.md), `cargo xtask actionable-gap-outcomes` | Source-edit-free packet generation only; agents or developers write the test outside RIPR and then attach a receipt. |
| Repo-scoped public badges | `usable alpha` | README, crate page, extension store, checked badge endpoints | [Badge policy](../BADGE_POLICY.md), [verification](../VERIFICATION.md), `cargo xtask check-badge-diff-policy`, `cargo xtask check-badge-endpoints` | Public badges count unresolved actionable canonical repair items; seam-native inventory remains internal analyzer-health pressure. They must not imply PR-local test adequacy, full test adequacy, runtime mutation confirmation, coverage, or merge approval. |
| PR-local evidence and gates | `usable alpha` | PR summaries, artifacts, optional gate decision | [Blocking readiness](../BLOCKING_READINESS.md), [calibrated gate policy](../CALIBRATED_GATE_POLICY.md), `cargo xtask check-output-contracts` | Advisory by default; only explicit gate-decision artifacts own configured pass/fail authority. |
| Source-of-truth artifact graph | `stable building block` | Source-of-truth docs, proposal/spec ledger, and xtask validator | [Source-of-truth proposal](../proposals/RIPR-PROP-0015-source-of-truth-control-plane.md), [source-of-truth spec](../specs/RIPR-SPEC-0060-source-of-truth-stack.md), `cargo xtask check-doc-artifacts`, `cargo xtask check-support-tiers` | Validates registered document artifact IDs, paths, statuses, kind/path fit, links, supersession, and support-tier proof-command drift. `cargo xtask pr-body --work-item <id>` and `cargo xtask closeout --goal <goal-id>` can generate unchecked scaffolds from active-goal metadata, but they do not infer support-tier impact, policy impact, completion, graph reports, or CI promotion. |
| TypeScript and JavaScript preview | `preview` | Opt-in CLI/report evidence, editor routing, and grouped generated CI | [Language adapter preview workflow](../LANGUAGE_ADAPTER_PREVIEW.md), [RIPR-SPEC-0027](../specs/RIPR-SPEC-0027-typescript-preview-static-facts.md), [Campaign 27 closeout](../handoffs/2026-05-13-campaign-27-closeout.md), [TypeScript preview completion closeout](../handoffs/2026-05-30-typescript-preview-completion-closeout.md), TypeScript fixture families | Syntax-first; preview-labeled; no default blocking; owner ids and owner kinds are fixture-backed for current function, component, method, class-method, arrow-function, and module-initializer shapes; strict preview actionability, repair-loop receipts, and route-quality metrics are advisory evidence only; static limits such as mocked modules are visible instead of hidden. |
| Python preview | `preview` | Opt-in CLI/report owner, test, assertion/oracle, probe, related-test, RIPR-stage, static-limit, editor, and generated-CI grouping evidence | [Language adapter preview workflow](../LANGUAGE_ADAPTER_PREVIEW.md), [Python repair routing proposal](../proposals/RIPR-PROP-0017-python-repair-routing-lane.md), [RIPR-SPEC-0028](../specs/RIPR-SPEC-0028-python-preview-static-facts.md), [ADR 0009](../adr/0009-python-parser-substrate.md), [Campaign 27 closeout](../handoffs/2026-05-13-campaign-27-closeout.md), Python owner/test, assertion/oracle, probe, related-test, RIPR evidence, repair-class discriminator, and static-limit fixture families | Owner, test, assertion/oracle, core probe, conservative related-test, syntax-first RIPR evidence, selected repair-class missing-discriminator facts, and fail-closed static-limit facts are fixture-backed; generated CI grouping remains advisory and opt-in. The repair-routing lane defines the repair-card, verify-command, and receipt evidence needed before Python can move beyond preview. |
| Editor preview language routing | `preview` | VS Code/LSP | [Language adapter preview workflow](../LANGUAGE_ADAPTER_PREVIEW.md), [Lane 3 tracker](../lanes/LANE_3_EDITOR_LSP.md), [RIPR-SPEC-0036](../specs/RIPR-SPEC-0036-editor-preview-routing.md), [RIPR-SPEC-0037](../specs/RIPR-SPEC-0037-editor-preview-static-limit-projection.md), [Campaign 27 closeout](../handoffs/2026-05-13-campaign-27-closeout.md) | VS Code registers TypeScript/JavaScript/Python selectors and LSP diagnostics preserve preview metadata and static limits; `[languages]` remains the analysis gate and Rust editor behavior remains the default. |
| Language-aware generated CI grouping | `preview` | Generated GitHub workflow | [Language adapter preview workflow](../LANGUAGE_ADAPTER_PREVIEW.md), [RIPR-SPEC-0038](../specs/RIPR-SPEC-0038-generated-pr-ci-review-workflow.md), [Lane 4 tracker](../lanes/LANE_4_PR_CI_REVIEW.md), [Campaign 27 closeout](../handoffs/2026-05-13-campaign-27-closeout.md) | The generated summary groups TypeScript/Python advisory evidence only when `[languages]` enables preview adapters; Rust-default output and gate authority remain unchanged. |
| Preview evidence policy promotion | `deferred` | Policy reports and future promotion packets | [Preview evidence policy boundary](../specs/RIPR-SPEC-0030-preview-evidence-policy-boundary.md), [preview promotion criteria](../policy/PREVIEW_PROMOTION_CRITERIA.md), [policy readiness closeout](../handoffs/2026-05-12-policy-readiness-closeout.md) | Preview evidence is visible and advisory by default; it is not gate, RIPR Zero, or baseline-check eligible without later explicit promotion and the required criteria. |

## How To Read A Claim

Use the tier with the surface:

```text
usable + Rust gap repair loop:
  safe to try as the end-to-end advisory workflow: repair one named Rust gap,
  verify movement, and keep the receipt.

usable alpha + generated CI:
  safe to try in advisory PR workflows, but not a default merge gate.

preview + TypeScript:
  safe to evaluate when explicitly enabled, but not a parity claim with Rust.

preview + Python:
  useful for opt-in syntax-first evidence, including owner, test, assertion,
  probe, related-test, RIPR-stage, selected repair-class discriminator, and
  static-limit facts. Fully working Python means the repair-card,
  verify-command, and receipt loop in RIPR-PROP-0017, not parser existence
  alone.

preview + editor routing:
  useful for opt-in editor projection, but not a Rust maturity or runtime
  adequacy claim.

stable building block + source-of-truth artifact graph:
  safe to rely on registered proposal/spec artifact links being mechanically
  checked by `cargo xtask check-doc-artifacts`, and safe to use
  `cargo xtask pr-body --work-item <id>` or
  `cargo xtask closeout --goal <goal-id>` as unchecked scaffolds; not a claim
  that support-tier or policy impact has been inferred, or that graph reports
  or CI promotion are generated or validated.
```

## Trust Boundaries

- Public badges are repo-scoped trust markers, not PR-local evidence.
- PR summaries and packets are diff-scoped advisory artifacts, not public repo
  badges.
- Gate decisions, when configured, own pass/fail authority; summaries and
  indexes do not.
- Runtime mutation testing is the execution-backed confirmation step; RIPR's
  normal output is static evidence.
- Preview-language evidence must stay opt-in, visibly labeled, and advisory
  until an explicit policy promotes it.
- Source-of-truth artifact validation proves the registered document graph,
  not the correctness of product behavior beyond the named proof commands.

## Next Adoption Steps

For first use, start with the [Quickstart](../QUICKSTART.md). For a
single-PR adoption proof, use the
[first successful PR workflow](../FIRST_PR_WORKFLOW.md). The shortest proof
loop is:

```text
ripr pilot --root .
-> read the top actionable test gap
-> add one focused test outside RIPR
-> capture an after snapshot
-> run ripr outcome
-> keep the receipt or PR summary
```

For PR review, start with the generated CI job summary and uploaded report
packet. For coding agents, start with
[`ripr agent status --root .`](../QUICKSTART.md#agent-or-reviewer-first-hour)
and then generate a bounded packet for one selected seam.

For TypeScript, JavaScript, or Python evaluation, start with
[Language adapter preview workflow](../LANGUAGE_ADAPTER_PREVIEW.md) so the
preview/advisory boundary is explicit before rollout.
