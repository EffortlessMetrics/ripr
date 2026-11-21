# Closeout: Bun UB TypeScript Preview

Date: 2026-06-04

Owner: Language adapter swarm

Linked specs:

- [RIPR-SPEC-0027](../specs/RIPR-SPEC-0027-typescript-preview-static-facts.md)
- [RIPR-SPEC-0062](../specs/RIPR-SPEC-0062-cross-language-oracle-graph.md)

Linked capability: `typescript_preview_static_facts`

Support-tier impact: none; TypeScript and JavaScript remain `preview`

Policy impact: none; no gate, badge, baseline, RIPR Zero, public repair
packet, or support-tier role

## Decision

The 0.8.1 Bun UB TypeScript slice is complete for the bounded advisory Blob /
`ArrayBuffer` route.

For that calibrated route, `ripr` can answer the review question:

```text
This Rust/FFI seam changed. Do Bun's TypeScript integration tests discriminate
the boundary that would catch the stable-byte bug?
```

The answer remains preview/advisory. It is useful for removing manual grep,
manual reinterpretation, and wrong-language test placement from this Bun review
path. It is not a TypeScript parity claim, runtime Bun evidence, mutation
evidence, a generated-test flow, or release authority.

The previous version of this handoff stopped at the advisory Bun UB profile
slice (#927). The patch-cut loop now also includes route-quality reporting, the
Bun UB calibration report command, dogfood receipts, the runbook, and later
fail-closed graph routing for additional Bun cross-language samples.

## Patch-Cut Answers

| Question | 0.8.1 answer |
| --- | --- |
| Can `ripr` credit Bun TypeScript tests for Blob SAB/RAB boundaries? | Yes, for the configured Bun Blob / `ArrayBuffer` route, as preview/advisory cross-language evidence. |
| Can it detect stripped missing branches? | Yes. Missing shared and missing resizable branches are named as missing discriminators. |
| Can it avoid mention-only false positives? | Yes. `maxByteLength` token mentions without a real observer stay `ts_mention_not_observer`. |
| Can it suggest the right TypeScript file? | Yes, for configured missing shared/resizable Blob rows: `test/js/web/fetch/blob.test.ts`. |
| Can it distinguish unknown bridge from no evidence? | Yes. Unknown bridge remains `bridge_unknown` with missing `binding_or_ffi_edge`, not `no_static_path` and not credited bridge evidence. |

## Useful States

`rust_ungripped_ts_discriminated`

The Rust seam is not gripped by Rust tests, but configured TypeScript evidence
has the shared/resizable discriminator facts, Blob/view input, stable-byte
oracle, and credited bridge leg. The advisory action is:

```text
no missing bridge discriminator
```

`rust_ungripped_ts_missing_discriminator`

The configured bridge exists, but a required branch is absent. The Blob /
`ArrayBuffer` route names the missing discriminator and, when placement evidence
exists, suggests:

```text
test/js/web/fetch/blob.test.ts
```

`ts_mention_not_observer`

The token exists but is not a witness. String/comment mentions, token-only
reads, smoke assertions, and partial Blob observers without byte/text/value
assertions do not become stable-byte witnesses.

`bridge_unknown`

TypeScript evidence may exist, but the binding or FFI graph leg is unresolved.
The result stays a named limitation with missing `binding_or_ffi_edge`; it does
not become `no_static_path`, a Rust test suggestion, or credited evidence.

## What Landed

| Surface | Evidence |
| --- | --- |
| Calibration corpus | `fixtures/typescript-bun-ub-calibration/corpus.json` pins the #31648-shaped known-good Blob route, stripped shared/resizable variants, stripped-both, partial-observer, mention-only, and bridge-unknown controls. |
| TypeScript facts and oracles | `crates/ripr/src/analysis/language/typescript.rs` extracts the Bun ArrayBuffer facts and separates stable byte/text/value observers from weak, smoke, snapshot, byte-read, and token-only evidence. |
| Bun bridge hints and grip projection | The bounded Bun Blob profile maps `src/jsc/Blob.rs`, `Blob::from_js_without_defer_gc`, `array_buffer.shared || array_buffer.resizable`, and `test/js/web/fetch/blob.test.ts` into preview-only cross-language findings. |
| Placement ranking | Configured missing shared/resizable discriminator rows suggest the TypeScript Blob test file and keep bridge-unknown, mention-only, partial-oracle, and target-unresolved rows at `suggested_test_file=not_applicable`. |
| Cross-language oracle graph | `fixtures/cross-language-oracle-graph-corpus/corpus.json` and SPEC-0062 require raw refs for Rust seam, boundary, binding/FFI edge, external TypeScript callsite, and external oracle before crediting the route. |
| Route-quality report | Readiness and evidence-quality scorecard outputs summarize complete advisory witnesses, missing discriminators, unknown bridges, mention-only limitations, panic-boundary limitations, and public packet exclusions. |
| Bun UB calibration report | `cargo xtask bun-ub-calibration` writes `target/ripr/reports/bun-ub-calibration.{json,md}` with observed state, missing discriminators, missing graph legs, suggested file, authority boundary, and `repair_packet_ready=false`. |
| Dogfood receipts | `fixtures/bun-ub-cross-language-dogfood/corpus.json` and `cargo xtask dogfood` record #31648-style known-good, stripped-resizable, mention-only, bridge-unknown, partial-oracle, and Bun FFI panic-boundary limitation receipts. |
| Operator runbook | [Bun UB TypeScript Preview Runbook](../BUN_UB_TYPESCRIPT_PREVIEW_RUNBOOK.md) documents the copyable advisory loop, actual command surface, state interpretation, receipts, and non-claims. |

## PR Chain

| PR | Slice |
| --- | --- |
| [#915](https://github.com/EffortlessMetrics/ripr-swarm/pull/915) | Bun Blob / `ArrayBuffer` calibration corpus |
| [#917](https://github.com/EffortlessMetrics/ripr-swarm/pull/917) | Bun `ArrayBuffer` discriminator facts |
| [#918](https://github.com/EffortlessMetrics/ripr-swarm/pull/918) | Stable-byte TypeScript oracle classification |
| [#919](https://github.com/EffortlessMetrics/ripr-swarm/pull/919) | Bun bridge hint evidence |
| [#920](https://github.com/EffortlessMetrics/ripr-swarm/pull/920) | Bridge verdict wording |
| [#922](https://github.com/EffortlessMetrics/ripr-swarm/pull/922) | Cross-language grip states and output projection |
| [#925](https://github.com/EffortlessMetrics/ripr-swarm/pull/925) | TypeScript observer placement ranking |
| [#927](https://github.com/EffortlessMetrics/ripr-swarm/pull/927) | Advisory Bun UB profile config |
| [#945](https://github.com/EffortlessMetrics/ripr-swarm/pull/945) | Cross-language oracle graph corpus |
| [#946](https://github.com/EffortlessMetrics/ripr-swarm/pull/946) | TypeScript discriminator witness routes and missing graph-leg projection |
| [#948](https://github.com/EffortlessMetrics/ripr-swarm/pull/948) | Unknown bridge witnesses remain limitations unless `binding_edge` is credited |
| [#954](https://github.com/EffortlessMetrics/ripr-swarm/pull/954) | Bun UB calibration report command |
| [#973](https://github.com/EffortlessMetrics/ripr-swarm/pull/973) | Bun Blob witness dogfood receipts |
| [#974](https://github.com/EffortlessMetrics/ripr-swarm/pull/974) | Bun FFI panic-boundary limitation fixture |
| [#975](https://github.com/EffortlessMetrics/ripr-swarm/pull/975) | Bun FFI panic-boundary dogfood receipt |
| [#976](https://github.com/EffortlessMetrics/ripr-swarm/pull/976) | Advisory Bun UB TypeScript preview runbook |

## Operator Loop

The patch-cut loop is now complete:

```text
fixture
-> analyzer facts
-> bridge / graph route
-> output projection
-> route-quality report
-> calibration report
-> dogfood receipt
-> runbook
-> closeout
```

The Bun operator can use the runbook and receipts to decide whether a changed
Rust stable-byte seam is:

- TypeScript-discriminated through the configured bridge;
- missing a named TypeScript discriminator;
- only a token mention;
- blocked by an unknown binding or FFI edge.

## Validation Commands

Focused patch-line validation:

```bash
cargo test -p xtask cross_language_oracle_graph_corpus_cases_are_checked -- --test-threads=1
cargo test -p xtask typescript_bun_ub_calibration_cases_are_checked -- --test-threads=1
cargo test -p xtask bun_ub_calibration_report_summarizes_calibrated_states -- --test-threads=1
cargo test -p xtask dogfood_bun_ub_cross_language_receipts_are_checked -- --test-threads=1
cargo test -p xtask evidence_quality_scorecard_summarizes_cross_language_oracle_route_quality -- --test-threads=1
cargo test -p ripr typescript_preview_card_projects_bun_cross_language_grip -- --test-threads=1
cargo xtask bun-ub-calibration
cargo xtask dogfood
cargo xtask check-output-contracts
cargo xtask check-static-language
cargo xtask check-traceability
cargo xtask check-capabilities
cargo xtask check-goals
cargo xtask goals next
cargo xtask check-doc-index
cargo xtask markdown-links
cargo xtask check-pr
git diff --check
```

## Claim Boundary

This closeout permits this claim:

```text
For calibrated Bun Blob / ArrayBuffer stable-byte seams, ripr can provide
advisory TypeScript preview evidence that distinguishes complete
cross-language discriminators, missing shared/resizable discriminators,
mention-only tokens, and unknown bridge routes, and can suggest the configured
TypeScript Blob test file for missing discriminator rows without producing a
public repair packet.
```

This closeout also permits a narrower negative claim:

```text
For the Bun FFI negative-offset panic-boundary sample, ripr keeps public
reachability plus an unresolved external panic oracle as a named static
limitation instead of suggesting a misleading Rust or TypeScript test target.
```

This closeout does not permit claims that:

- TypeScript or JavaScript preview is Rust-parity analysis.
- The Bun binding graph is complete beyond configured profile-backed samples.
- Runtime Bun, Jest, Vitest, `tsc`, `tsserver`, Miri, provider calls, or
  mutation execution ran.
- `ripr` generated tests or edited Bun source.
- A public repair packet is ready from preview or limitation evidence.
- Default CI gates, public badges, baselines, RIPR Zero, support tiers,
  release records, publishing, signing, marketplace, or install docs changed.
- A TypeScript discriminator proves UB or runtime behavior.

## What Remains Preview

- TypeScript and JavaScript language adapters remain opt-in preview.
- Bridge hints are bounded configured evidence, not a full Bun binding graph.
- Cross-language witnesses remain advisory until public actionability fields
  are explicit: verify command, receipt command, allowed edit surface,
  must-not-change guardrails, confidence, and raw evidence refs.
- Additional Bun seam families need their own fixture families, stripped
  variants, false-positive controls, route-quality rows, and dogfood receipts.

## Stronger Claim Criteria

Any stronger claim needs a separate preview-promotion or release packet that
includes:

- raw evidence refs for the Rust seam, boundary, binding/FFI edge, external
  callsite, and external oracle;
- bridge calibration beyond the bounded profile;
- false-positive and false-negative review receipts;
- route-quality and dogfood receipts;
- explicit rollback and policy-owner signoff;
- runtime red/green, Miri/model, or mutation evidence when the claim depends on
  runtime behavior rather than static advisory evidence.

## Archive Updates

- `.ripr/traceability.toml` maps `RIPR-SPEC-0027` and SPEC-0062 evidence to the
  Bun calibration corpus, cross-language oracle graph corpus, dogfood receipts,
  runbook, focused tests, and this closeout.
- `metrics/capabilities.toml` records this closeout as the Bun UB 0.8.1
  support boundary under TypeScript preview static facts.
- `docs/handoffs/README.md`, `docs/DOCUMENTATION.md`, and the support-tier
  evidence row link this closeout for future operators.

## Patch Recommendation

The 0.8.1 TypeScript/Bun patch is defensible once the focused validation commands
above are green on the patch branch and the normal `check-pr` guard passes.
Cutting, tagging, publishing, signing, marketplace, install-doc, or public
release-record work still requires explicit release authorization.
