# Exact-candidate full-population verdict measurement

Source: `a157a1d33fca88187e532cba4f1e3f49b97b0f02`, tree `7d17ca7d4dcbd420cbfbd13426b8f6f580cc3773`, repository EffortlessMetrics/ripr-swarm. Fetched full SHA/tree, clean detached checkout, no local patch. Intermediate preparation only: no qualification transfers to a successor or selected release.

All 1,698 scoring definitions and all four corpus directories are unchanged from the exact 17ba8c4e measurement. Baselines, truth labels, expected rows and missed-credit ledgers are unchanged. Each language is fully populated, not a selected favorable subset.

| Language | Cases | False actionable | False exposed / silent | Abstained | Missed credit / discriminated truth |
|---|---:|---:|---:|---:|---:|
| Rust | 846 | 0/433 | 0/413 each | 334/846 | 177/433 |
| TypeScript | 373 | 0/212 | 0/161 each | 50/373 | 19/212 |
| Python | 385 | 0/181 | 0/204 each | 83/385 | 12/181 |
| Perl | 94 | 0/36 | 0/58 each | 49/94 | 30/36 |
| Total | 1,698 | 0/862 | 0/836 each | 516/1,698 | 238/862 |

All four full reports have zero false verdicts and zero contradictions. Rust contradiction population is 1,035 row/probe pairs; oracle accuracy remains 49/49 with zero overclaims. Coverage is unchanged: Rust330/330, TypeScript162/164, Python62/62, Perl32/33.

## Complete comparable movements

Against exact 17ba8c4e, the only observed verdict/classification/outcome change is `spec0228-keyed-getter-same-key`: false-actionable/gap/weakly_exposed becomes ideal/credited/exposed. Rust ideal count increases 511→512. No new or recovered abstention; every other language outcome is unchanged. This verifies the previously reported same-key regression is restored on this exact source.

Against prior 62234ac13, the 24 Rust false-actionable rows become 19 ideal and five abstained; no false verdict remains. Five honest limitations remain: cell-0005-wrapper-field-binding, grid-flow-field-opaque, shape-refcell-interior-mutation, spec-b-0158-strip-prefix-absent, tokens-long-flag-strip-prefix. The additional `grid-iter-property` remains ideal/gap→abstained/limited; its truth is partially_discriminated and it is excluded from discriminated-truth missed-credit numerators. Those six transitions exactly explain total abstentions510→516, without offsetting gains. The previous seven additional abstentions also remain. Against retained984 committed observations, thirteen Rust ideal→abstained transitions remain and no false-verdict transition remains. Historical sides are retained reports, not newly executed old binaries.

All rows, truth/verdict enums, origin cohorts, Perl producer/edited-producer cohorts, comparable denominator identities and complete changed rows from17ba8 are retained in test-records.json. These authored/producer-corpus rates are not estimates of general real-world accuracy.

## Gate result and claim limits

The build passed. `xtask verdict-corpus check-all` exits1 because twelve Rust discriminated cases lack accepted missed-credit entries and expected rows drift. No ledger entry was added and no row was blessed. The twelve are: accounts-balance-add, cell-0005-wrapper-field-binding, grid-delete-exact, grid-flow-field-opaque, hex-from-hex-odd-variant, shape-op-add-impl, shape-refcell-interior-mutation, spec-b-0158-strip-prefix-absent, spec0228-field-write-direct-read, spec0228-field-write-enum-variant, spec0228-field-write-getter-read, tokens-long-flag-strip-prefix. Removing wrong advice is not restoration of discrimination. The grid-iter-property usefulness loss remains mechanically explained but unadjudicated.

The pre-existing Perl incomparable-baseline re-admission/abstention decision is separate and unresolved; no baseline was rewritten or silently re-admitted. No new runtime-mutant replay, broad trust suite, central full-library suite, file-policy pass, accepted-main wording/golden refresh, final release selection or promotion proof is claimed.

Only sanitized minimal test records, hashes and aggregate receipts are published. Raw analyzer outputs, private paths, environment/process metadata, executables and full logs remain private. All release-selection and promotion holds remain. No integration branch, source code, label, expectation, baseline, security setting, CI dispatch or release was changed by this lane. Workflow triggers were inspected before the non-triggering handoff-ref publication; prior Actions proxy403 was not retried or bypassed.

## Terminal custody result

All four JSON reports repeat identically. Independent capture executed all1,698 native invocations with exit0, no dropped/skipped cases and no collector failure. Retained-byte SHA256, exact diff identities, expected-language routing and completeness audit passed. Outcomes retain157 partial analyses and13 legitimate no-behavioral-candidate cases in the population. Copied actor bytes were hash-verified before/after against their sources; both analyzer versions embed the exact candidate SHA. All commands terminated, with no resource guard triggered.
