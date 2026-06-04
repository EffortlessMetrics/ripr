# Closeout: Lane 1 Post-0.8 Evidence-To-Repair Operating Loop

Date: 2026-06-04

Branch: `campaign-lane1-post-08-closeout`

Closed goal: `lane1-post-08-operating-loop`

Active manifest state: `status = "closed"` and `no_current_goal = true`

Archived manifest:
`.ripr/goals/archive/2026-06-04-lane1-post-08-operating-loop.toml`

## Decision

The post-0.8 Lane 1 operating loop is closed for the current repo state.

The loop now has one coherent path from large or mixed-language static evidence
to operator-facing repair and limitation state:

```text
changed surface or repo evidence
-> complete repair packet or named limitation
-> attempt/receipt/outcome ledger
-> route-quality summaries
-> user-surface projection
```

The important product boundary is unchanged. `ripr` gives static,
advisory evidence and bounded repair intent. It does not run mutation
execution, prove runtime adequacy, edit source, call providers, change public
badge semantics, or make default CI blocking decisions.

## What Landed

| Area | Result |
| --- | --- |
| Queue hygiene | The active queue was reconciled against live PR state, stale/done items were cleared, and post-0.8 work was moved into one manifest. |
| Review comments | Review-comment rows carry source file/line context or explicit unknown-source limitations. |
| Large-repo limits | Large seam-cache skips and large-run limits surface as named limited states instead of silent loss. |
| Diff-first mode | `ripr diff --base <ref> --head <ref>` can emit changed-file and changed-seam evidence before broader repo context. |
| Cross-language fail-closed routing | Rust seams that appear externally tested stay limitations until external oracle paths are explicit. |
| Language-aware placement | Binding, FFI, and externally tested seams avoid unrelated Rust repair placement unless observer evidence supports it. |
| Cross-language oracle graph | The bounded SPEC-0062 Bun Blob route has graph-leg evidence, missing-leg routing, route-quality summaries, and a Bun UB calibration receipt while remaining preview/advisory. |
| Repair-packet guidance | Repair packets require canonical identity, repair kind, target shape, verify command, receipt command, allowed edit surface, must-not-change boundaries, confidence, and raw evidence refs. |
| Attempt ledger | Attempt/readiness surfaces preserve not-attempted, attempted-without-receipt, receipt-present, improved, unchanged, regressed, resolved, stale, gap-mismatch, latest-attempt, and orphan-receipt states. |
| Dogfood attempts | The real-repair-attempts corpus records improved, resolved, unchanged, and attempted-without-receipt cases instead of curating only wins. |
| Route quality | Readiness and scorecard outputs summarize repair-route, language-route, limitation-route, missing-field, failing-route, and cross-language oracle quality. |
| Surface alignment | Surface-projection dogfood keeps badge, LSP/editor, PR comment, and CI examples on the same canonical repair or limitation state, receipt state, runtime state, and raw-finding non-claims. |

## What Users Can Trust

- Complete public repair packets are deliberately narrow: they carry stable
  `canonical_gap_id`, repair kind, target shape, verify and receipt commands,
  allowed edit surface, must-not-change boundaries, confidence, and raw evidence
  refs.
- Incomplete cross-language, binding, FFI, or external-oracle paths fail closed
  as limitations instead of becoming wrong-language repair packets.
- Large-repo and diff-first runs preserve runtime status such as full,
  diff-complete, limited, sampled, stale, timeout, incomplete, runner failure,
  or cache-limited rather than presenting partial data as full truth.
- Attempt and route-quality reports preserve non-success states, including
  unchanged, regressed, missing receipt, and limitation-route backlog items.
- User-facing surfaces consume canonical repair or limitation state; raw
  findings remain supporting evidence, not the unit of product truth.

## Still Advisory

- TypeScript, JavaScript, Python, binding, FFI, and cross-language evidence
  remains bounded by its explicit preview/support tier.
- The Bun Blob TypeScript route is calibrated and useful for advisory review,
  but it is not a full Bun binding graph, runtime Bun proof, generated test,
  badge input, gate, baseline, RIPR Zero claim, or support-tier promotion.
- Route-quality and attempt-ledger reports guide prioritization; they do not
  prove correctness or replace human review.
- `ripr-swarm` remains the development/control-plane repo. Source `ripr`
  remains release, signing, publishing, and distribution authority.

## Non-Actionable Boundaries

- Static limitations are not repair packets.
- Raw findings are not independent work items.
- Missing bridge, binding, FFI, external oracle, verify command, receipt
  command, allowed edit surface, or raw evidence refs block public repair
  packet projection.
- Unknown bridge confidence remains `bridge_unknown` or an equivalent named
  limitation, not `no_static_path` and not credited external evidence.
- No default CI gate, badge semantic switch, provider integration, autonomous
  edit loop, mutation execution, generated tests, source release, publish,
  signing, marketplace, or install-doc change is included in this closeout.

## Validation

Latest closeout PR validation:

```bash
rtk cargo xtask check-goals
rtk cargo xtask goals next
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-doc-roles
rtk git diff --check
```

The immediately preceding surface-alignment PR also passed:

```bash
rtk cargo test -p xtask dogfood_surface_projection_alignment -- --test-threads=1
rtk cargo test -p xtask dogfood_user_surface_projection_alignment -- --test-threads=1
rtk cargo xtask dogfood
rtk cargo xtask check-goals
rtk cargo xtask goals next
rtk cargo xtask check-output-contracts
rtk cargo xtask check-static-language
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-doc-roles
rtk git diff --check
```

Local `rtk cargo xtask check-pr` repeatedly timed out after ten minutes in the
disposable tracker worktrees without diagnostics. PR CI provided the final
green branch signal for the preceding tracker PRs.

## PR Chain

| PR | Result |
| --- | --- |
| #924 | Selected and reconciled the post-0.8 real-repo trust queue. |
| #926 | Added review-comment source locations. |
| #928, #934, #935, #936 | Surfaced and hardened large-repo cache/runtime limited states. |
| #930 | Routed unresolved cross-language oracle paths as limitations. |
| #938, #939, #940, #941, #942 | Closed language-aware target placement navigation. |
| #943, #945, #946, #948 | Added the bounded SPEC-0062 oracle graph corpus, TS witness routes, and unknown-bridge routing. |
| #954 | Added the Bun UB calibration report receipt. |
| #959 | Closed repair-packet guidance quality from existing packet evidence. |
| #961 | Closed attempt-ledger outcome hardening. |
| #962 | Closed real repair/analyzer-attempt dogfood. |
| #963 | Closed route-quality metrics. |
| #964 | Closed surface canonical-state alignment. |

## Open Work

No work item remains selected in `.ripr/goals/active.toml`.

Future work should be selected from live repo state, not by continuing this
closed manifest. Expected future themes remain outside this closeout:

- broader cross-language oracle graph work for #908/#910-style cases;
- more Bun UB seam families beyond the calibrated Blob / `ArrayBuffer` route;
- large-repo scalability beyond explicit limited-state reporting;
- support-tier promotion packets for preview-language loops only when evidence,
  false-positive review, rollback, and policy-owner signoff justify them;
- any source release or hotfix publication, which requires explicit release
  authorization in source `ripr`.

## Archive Updates

- Active goal manifest closed: `.ripr/goals/active.toml`
- Archived manifest:
  `.ripr/goals/archive/2026-06-04-lane1-post-08-operating-loop.toml`
- Handoff:
  `docs/handoffs/2026-06-04-lane1-post-08-operating-loop-closeout.md`
