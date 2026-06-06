# Closeout: Lane 1 Cross-Language Oracle Follow-Up

Date: 2026-06-04

Closed goal: `lane1-cross-language-oracle-followup`

Active manifest state: `status = "closed"` and `no_current_goal = true`

Archived manifest:
`.ripr/goals/archive/2026-06-04-lane1-cross-language-oracle-followup.toml`

## Decision

The selected #908/#910 follow-up campaign is closed for the current repo state.

The campaign did not solve generic cross-language oracle visibility. It added
measured, profile-backed slices beyond the original Bun Blob route and kept the
trust boundary intact:

```text
complete configured external witness -> preview/advisory evidence
missing graph leg or unsafe target -> named limitation
preview or limitation evidence -> no public repair packet
```

No successor campaign is selected by this closeout. Future #908/#910 work should
start from live issue and artifact state, not by extending this closed manifest.

## What Landed

| Work item | Result |
| --- | --- |
| `fixtures/cross-language-copy-to-unshared-profile` | Selected #908/#910 as the follow-up and pinned a measured `copy_to_unshared` TypeScript-exercised Rust seam as a profile-backed `bridge_unknown` limitation with source locations, missing binding edge, repair route, unlock condition, raw evidence refs, and no public repair-packet fields. |
| `report/configured-cross-language-ts-placement` | Surfaced `test/js/web/fetch/blob.test.ts` only for configured Bun Blob missing-discriminator rows while keeping the result advisory: no public projection, verify command, receipt command, allowed edit surface, wrong Rust test target, or repair packet. |
| `analysis/cross-language-copy-to-unshared-bridge-route` | Added configured bridge evidence for the `copy_to_unshared` profile and credits the external TypeScript oracle only as a preview/advisory witness. |
| `analysis/bun-markdown-resizable-cross-language-profile` | Added the #951 `MarkdownObject::to_string` configured `Bun.markdown` profile and credits `test/js/bun/md/md-edge-cases.test.ts` only when resizable ArrayBuffer, configured bridge, callsite, and strong markdown oracle evidence are present. Weak markdown oracle evidence remains a named limitation. |
| `dogfood/bun-blob-witness-receipts` | Recorded checked Bun Blob cross-language dogfood receipts for complete advisory, missing-discriminator, bridge-unknown, and partial-oracle cases while preserving advisory-only authority, route-quality counters, non-claims, and no public repair packets. |
| `fixtures/bun-ffi-negative-offset-panic-boundary-profile` | Added the #950 Bun FFI `FFIObject::read` negative-offset panic-boundary profile as a named static limitation with source locations, FFI binding sample, missing negative-offset panic oracle, missing safe external observer target, unlock condition, raw evidence refs, and no suggested test file, verify command, receipt command, allowed edit surface, public projection eligibility, or repair packet. |
| `dogfood/bun-ffi-panic-boundary-receipt` | Recorded a checked Bun FFI negative-offset panic-boundary dogfood receipt for #950/#974 proving the route stays a named limitation with unresolved negative-offset oracle and safe observer target evidence. |

## Claim Boundary

This closeout permits this claim:

```text
RIPR has measured cross-language profile slices for configured Bun Blob,
copy_to_unshared, MarkdownObject, and FFI panic-boundary cases. Complete
configured witnesses remain preview/advisory, and unresolved graph or target
legs remain named limitations with no public repair packet.
```

This closeout does not permit claims that:

- #908 or #910 are fully resolved;
- RIPR has generic TypeScript, JavaScript, binding, or FFI oracle proof;
- Bun runtime, Jest, Vitest, `tsc`, `tsserver`, Miri, or mutation execution ran;
- generated tests, provider calls, autonomous edits, or source release actions
  were added;
- preview evidence is a public repair packet;
- wrong-language target placement is safe without explicit graph and observer
  evidence.

## Remaining Work

#908 and #910 remain open as broader cross-language oracle follow-ups. Future
work should select a new measured profile or a narrow analyzer route from live
issue state, with its own manifest or work item.

Expected future themes:

- more non-Blob Bun seam families with real binding/callsite/oracle samples;
- language-aware target placement only when external target evidence is
  explicit;
- route-quality learning from additional dogfood receipts;
- eventual promotion from preview/advisory only when every public repair-packet
  field, verify command, receipt command, raw evidence ref, and edit cage is
  explicit.

## Validation

Closeout validation:

```bash
rtk cargo xtask check-goals
rtk cargo xtask goals next
rtk cargo xtask check-doc-index
rtk cargo xtask markdown-links
rtk cargo xtask check-static-language
rtk cargo xtask check-doc-roles
rtk cargo xtask check-pr-shape
rtk git diff --check
```

The immediately preceding campaign PRs also carried focused route proof,
including:

```bash
rtk cargo test -p xtask cross_language_oracle_graph -- --test-threads=1
rtk cargo test -p ripr cross_language -- --test-threads=1
rtk cargo test -p xtask evidence_quality_scorecard_summarizes_cross_language_oracle_route_quality -- --test-threads=1
rtk cargo test -p xtask dogfood_bun_ub_cross_language_receipts_are_checked -- --test-threads=1
rtk cargo xtask dogfood
rtk cargo xtask check-output-contracts
rtk cargo xtask check-static-language
```

## Archive Updates

- Active goal manifest closed: `.ripr/goals/active.toml`
- Archived active goal manifest:
  `.ripr/goals/archive/2026-06-04-lane1-cross-language-oracle-followup.toml`
- Handoff:
  `docs/handoffs/2026-06-04-lane1-cross-language-oracle-followup-closeout.md`
