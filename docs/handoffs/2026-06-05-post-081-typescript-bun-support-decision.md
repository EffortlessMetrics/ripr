# Post-0.8.1 TypeScript/Bun Support Decision

Date: 2026-06-05

## Decision

TypeScript and JavaScript remain opt-in preview.

For calibrated Bun stable-byte cases, RIPR can provide advisory cross-language
evidence that distinguishes:

- `rust_ungripped_ts_discriminated`
- `rust_ungripped_ts_missing_discriminator`
- `ts_mention_not_observer`
- `bridge_unknown`
- named static limitation states

This is a useful Bun UB review signal, not a support-tier promotion.

## Evidence Considered

- The 0.8.1 TypeScript/Bun patch proof records calibrated Blob / ArrayBuffer,
  `copy_to_unshared`, MarkdownObject, and FFI panic-boundary preview evidence.
- `cargo xtask bun-ub-preview-summary` gives the one-screen advisory summary.
- Configured TypeScript preview cards include Bun cross-language advisory
  packets with stop conditions and `repair_packet_ready=false`.
- Stable-byte proof mode is projected as an advisory planning label only.
- node:fs scalar write and Bun.write are manifest-only named limitations.
- `cargo xtask configured-bridge-inventory` lists configured, bridge-unknown,
  manifest-only, and named-limitation surfaces without analyzer inference.
- `fixtures/bun-ub-cross-language-dogfood` records live-shaped receipts for
  configured, missing, mention-only, bridge-unknown, and named-limitation cases.
- The Bun UB runbook now documents the `ripr.toml`, `ripr doctor`, and
  `ripr check` first-run loop and points to the summary, inventory,
  calibration, and dogfood receipts.

## Current Claim

RIPR can help a Bun operator answer:

```text
This Rust/FFI stable-byte seam changed.
Does configured TypeScript integration evidence discriminate the boundary?
```

The answer is advisory and calibrated for the currently modeled Bun stable-byte
routes. It can say the TypeScript route is discriminated, a named discriminator
is missing, a token is only a mention, a bridge is unknown, or a named static
limitation blocks actionability.

## Non-Claims

This decision does not claim:

- TypeScript or JavaScript stable support.
- Bun UB proof.
- Runtime Bun, Jest, Vitest, `tsc`, `tsserver`, Miri, or mutation execution.
- Generated tests.
- Source edits.
- Default gates, badges, baselines, or RIPR Zero contribution.
- Public repair packets from TypeScript/Bun preview evidence.
- A full Bun binding graph.
- Generic cross-language support for every mixed TypeScript/Rust repository.

## Promotion Bar

Any stronger TypeScript/Bun claim needs a separate accepted promotion contract.
At minimum, that later packet would need to specify:

- the exact support tier being requested;
- the additional Bun surfaces covered beyond the current calibrated routes;
- bridge or binding evidence that is no longer manifest-only;
- false-positive and false-negative review evidence across live Bun changes;
- how runtime, mutation, Miri, or red/green proof is represented when required;
- why public repair-packet, gate, badge, baseline, or support-tier authority is
  safe for that narrower surface.

Until then, the safe decision is preview/advisory only.

## Campaign State

The cross-language evidence router UX campaign has reached the planned
post-0.8.1 decision point. No successor campaign is selected by this handoff.

Future work should start from a new source-of-truth selection instead of
continuing this campaign by chat history.
