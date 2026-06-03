# Closeout: Bun UB TypeScript Preview

Date: 2026-06-03

Owner: Language adapter swarm

Linked spec: [RIPR-SPEC-0027](../specs/RIPR-SPEC-0027-typescript-preview-static-facts.md)

Linked capability: `typescript_preview_static_facts`

Support-tier impact: none; TypeScript and JavaScript remain `preview`

Policy impact: none; no gate, badge, baseline, RIPR Zero, or support-tier role

## Decision

The 0.8.1 Bun UB slice is complete for one bounded advisory route: the Bun
Blob / `ArrayBuffer` seam around
`array_buffer.shared || array_buffer.resizable`.

For that route, `ripr` can answer the review question:

```text
This Rust/FFI seam changed. Do Bun's TypeScript integration tests discriminate
the boundary that would catch the UB?
```

The answer remains preview/advisory. It is useful for removing manual
grep-and-reinterpretation from this Bun review path, not for claiming
TypeScript parity with Rust, runtime adequacy, coverage adequacy, or default
merge authority.

## What Landed

| Surface | Evidence |
| --- | --- |
| Calibration fixture | `fixtures/typescript-bun-ub-calibration/corpus.json` pins the known-good Blob / shared+resizable case, stripped shared/resizable variants, mention-not-observer control, bridge-unknown state, and TypeScript test placement. |
| TypeScript discriminator facts | `crates/ripr/src/analysis/language/typescript.rs` extracts `SharedArrayBuffer`, resizable `ArrayBuffer` through `maxByteLength`, `ArrayBuffer.resize(...)`, typed-array/DataView views, view-backed `Blob(...)` input, and `blob.arrayBuffer()` observers. |
| Stable-byte oracle classification | The TypeScript adapter distinguishes stable byte/text/value observers from smoke, snapshot, byte-read, and token-only evidence so a `maxByteLength` mention cannot stand in for a Blob observer. |
| Bun bridge hints | The bounded internal Bun Blob profile maps `src/jsc/Blob.rs`, `Blob::from_js_without_defer_gc`, the Rust boundary text, and `test/js/web/fetch/blob.test.ts` into evidence-only `configured_hint` or `bridge_unknown` lines. |
| Cross-language grip states | Changed Rust Blob boundary lines project advisory states for `rust_ungripped_ts_discriminated`, `rust_ungripped_ts_missing_discriminator`, `ts_mention_not_observer`, and `bridge_unknown`. |
| TypeScript placement ranking | Missing-discriminator cases rank `test/js/web/fetch/blob.test.ts` first with a reason tied to the missing shared/resizable discriminator, while keeping Rust-test placement suppressed. |
| Opt-in profile | `[profiles.bun_ub]` records TypeScript-family test roots and a repo-relative bridge-hints path for operators; the profile is absent by default and does not enable runtime Bun, `tsc`, `tsserver`, generated tests, gates, badges, baselines, RIPR Zero, or support-tier promotion. |

## PR Chain

| PR | Slice |
| --- | --- |
| [#915](https://github.com/EffortlessMetrics/ripr-swarm/pull/915) | Bun Blob / `ArrayBuffer` calibration corpus |
| [#917](https://github.com/EffortlessMetrics/ripr-swarm/pull/917) | Bun `ArrayBuffer` discriminator facts |
| [#918](https://github.com/EffortlessMetrics/ripr-swarm/pull/918) | Stable-byte oracle classification |
| [#919](https://github.com/EffortlessMetrics/ripr-swarm/pull/919) | Bun bridge hint evidence |
| [#920](https://github.com/EffortlessMetrics/ripr-swarm/pull/920) | Bridge verdict wording |
| [#922](https://github.com/EffortlessMetrics/ripr-swarm/pull/922) | Cross-language grip states and output projection |
| [#925](https://github.com/EffortlessMetrics/ripr-swarm/pull/925) | TypeScript observer placement ranking |
| [#927](https://github.com/EffortlessMetrics/ripr-swarm/pull/927) | Advisory Bun UB profile config |

## Proof Executed

Representative focused proof across the lane:

```bash
cargo test -p xtask typescript_bun_ub_calibration_cases_are_checked -- --test-threads=1
cargo test -p ripr bun_bridge_hint -- --test-threads=1
cargo test -p ripr changed_rust_blob_boundary_projects -- --test-threads=1
cargo test -p ripr typescript_preview_card_projects_bun_cross_language_grip -- --test-threads=1
cargo test -p ripr config::tests::bun_ub_profile -- --test-threads=1
cargo test -p ripr cli::commands::tests::doctor -- --test-threads=1
cargo xtask check-capabilities
cargo xtask check-traceability
cargo xtask check-doc-index
cargo xtask check-pr
```

Final PR #927 passed local `cargo xtask check-pr` on the branch, GitHub PR
checks including `Ripr Rust Small Result`, and post-merge `cargo xtask
check-pr` on fresh `origin/main` at `1d4f7e98`.

## Claim Boundary

This closeout permits the following claim:

```text
For the configured Bun Blob / ArrayBuffer route, RIPR can produce advisory
TypeScript preview evidence that distinguishes complete discriminators,
missing shared/resizable discriminators, token-only mentions, and unknown
bridge confidence, and can suggest the configured TypeScript observer file
without producing a public repair packet.
```

This closeout does not permit claims that:

- TypeScript or JavaScript preview is a Rust-parity analyzer.
- The Bun bridge map is complete beyond the bounded Blob route.
- Runtime Bun, `tsc`, `tsserver`, Jest, Vitest, or mutation execution ran.
- Generated tests, source edits, repair packets, default CI gates, badges,
  baselines, RIPR Zero, or support tiers were changed.
- A TypeScript discriminator confirms the Rust behavior at runtime.

## Remaining Work

- Parse or generate repo-specific bridge hints only after a separate contract
  defines bridge-hint syntax, trust boundaries, validation, and raw evidence
  references.
- Add more Bun UB seam families only as separate calibration fixtures with
  stripped variants and false-positive controls.
- Promote any TypeScript/Bun evidence toward a stronger tier only with a
  separate preview-promotion packet that includes raw evidence references,
  bridge calibration, false-positive review, route-quality receipts, rollback,
  and policy-owner signoff.
- Keep default generated CI, public badges, baselines, RIPR Zero, and gates
  unchanged until a separate explicit policy change lands.

## Archive Updates

- `.ripr/traceability.toml` now maps `RIPR-SPEC-0027` to the Bun calibration
  fixture, focused tests, and this closeout.
- `metrics/capabilities.toml` records this closeout as the Bun UB 0.8.1
  support boundary under TypeScript preview static facts.
- `docs/handoffs/README.md`, `docs/DOCUMENTATION.md`, and the support-tier
  proof row link this closeout for future operators.

## Next Recommended Goal

Continue the active real-repo trust readiness campaign. The Bun UB preview
route is now a bounded advisory tool; the next high-value product work is to
keep mixed-language and binding/FFI seams fail-closed unless their external
oracle path is explicitly visible.
