# Exact intermediate verdict measurement: 17ba8c4e

Repository: EffortlessMetrics/ripr-swarm. Exact source **17ba8c4e68ae07fc53a76e8013ac3c572988b5f0**, tree **b95cd9a8323c4847d27c42b3dd5c312350f01162**. This is the fetched intermediate correctness pin, not final selected-cut acceptance. No local patch or composed subject was used: the prior eight-file metadata repair is already integrated. No claim here transfers to a successor.

All corpus files and all 1,698 scoring definitions equal the prior 62234ac13 measurement and the retained 984c69a290 population. Corpus inventory SHA256 remains 40bebd79b5bf5e3587fa8653d1fc344c88e1ceb8f0daffa7d75bfe16bc481cad. Baselines, truth labels, expected rows and ledgers are unchanged. Production/xtask source changed, so prior analyzer results were not reused as current results.

| Language | Cases | False actionable | False exposed / false silent | Abstained | Missed credit / discriminated truth |
|---|---:|---:|---:|---:|---:|
| Rust | 846 | 1/433 | 0/413 each | 334/846 | 177/433 |
| TypeScript | 373 | 0/212 | 0/161 each | 50/373 | 19/212 |
| Python | 385 | 0/181 | 0/204 each | 83/385 | 12/181 |
| Perl | 94 | 0/36 | 0/58 each | 49/94 | 30/36 |
| Total | 1,698 | 1/862 | 0/836 each | 516/1,698 | 238/862 |

The only false-verdict row is **spec0228-keyed-getter-same-key**, a new false-actionable regression relative to the prior exact measurement. It was credited before and is now a gap. No false-exposed or false-silent row was observed in these corpora. Contradictions remain zero; Rust is 0/1035 row/probe pairs. Rust oracle-kind accuracy is 49/49, overclaim 0/49. Named spec coverage and floors are unchanged.

All four complete JSON reports repeat identically. Every selected case executed and was classified; no case was dropped, skipped or made ineligible. The independent CLI capture completed **1,698 native exit-0 invocations**, and its retained-byte, diff-identity, expected-language routing and completeness audit passed. Raw results include 157 disclosed partial analyses and 13 legitimate no-behavioral-candidate cases; these remain in the populations and do not become definitive exposure verdicts. Capture and audit both exited 0; there was no collector failure in this run.

## Same-definition comparisons

Against the previous 62234ac13 live measurement, Rust changes 24→1 false verdicts and 328→334 abstentions. All other languages' outcomes are unchanged. Exact movements:

- 19 prior false-actionable rows are now ideal/credited.
- Five prior false-actionable rows now abstain: cell-0005-wrapper-field-binding, grid-flow-field-opaque, shape-refcell-interior-mutation, spec-b-0158-strip-prefix-absent, tokens-long-flag-strip-prefix. Harmful advice is removed, but discrimination is not restored.
- grid-iter-property changes ideal→abstained (partially discriminated truth); this is an additional usefulness cost.
- spec0228-keyed-getter-same-key changes ideal/credited→false-actionable/gap.
- The previous seven additional abstentions all remain abstained.

Against retained 984c69a290 committed observations, Rust has 13 ideal→abstained and one ideal→false-actionable, with no definition changes. The older sides are retained observations, not newly rerun historical binaries. Origins, Perl producer versus edited-producer cohorts, every case's minimal test result and both complete comparison populations are preserved in test-records.json. Authored-case rates are not general real-world accuracy estimates.

## Concrete getter failure and isolated authority

An independent direct CLI invocation on the exact actor reproduces weak call-deletion and side-effect findings. Reach, infection, propagation and observation are yes, but discrimination is weak with observation_unverified, despite the same-key getter assertion. The existing source case is fixtures/rust-verdict-corpus/cases/spec0228-keyed-getter-same-key.json (RIPR-SPEC-0228 example 16).

A private unchanged fixture copy reproduces both weak findings. Renaming only the unrelated **Bin::count** method and its one test call restores both findings to exposed/yes; both CLI invocations exit 0. Shelf::restock, Shelf::count, their test, the changed line and case diff are unchanged. This is a static diagnostic fixture experiment, not a corpus edit, analyzer repair or fresh runtime-mutant replay.

The earliest newly added authority is classifier/evidence.rs's getter_reads_key callback: it now requires builder_override.rs::getter_dispatch_body before getter_plainly_reads_key. indexed_getter_body then requires owner_pin.rs::getter_dispatch_is_unambiguous, which calls other_definition_competes with no settled receiver. Its method-name competition includes a same-named inherent method on a different nominal type. The unrelated Bin::count therefore blocks Shelf::count. Source inspection and the controlled rename isolate this boundary. Do not delete genuine trait/unknown-dispatch exclusions; central owns the scoped repair.

Minimal existing-producer reproduction:

```
cargo xtask verdict-corpus report --cases spec0228-keyed-getter-same-key --out <owned-report>
```

The complete full-population runs and independent CLI reproduction were actually executed; this portable command is the bounded reproduction recipe for another checkout. No source fix was written by this lane.

## Gate status, privacy and limits

Tool build passed. Full check-all exits **1**, retaining the false row and expected-row/missed-credit drift. All four report-mode repeats exit 0; that establishes stability, not a passing quality gate. No expected row was blessed and no missed-credit entry was added. The Perl incomparable-baseline re-admission/abstention decision remains unresolved; its baseline and definition inputs are unchanged. It was not silently re-admitted or rerun as a broad trust suite.

The public packet contains only whitelisted minimal repair/test records: source/tree and actor hashes, case IDs, truth/verdict/outcome enums, aggregate counts, comparison populations and this compact diagnosis. **Raw analyzer JSON, environment/process metadata, private paths, full logs and executable bytes remain private.** No public raw archive or qualification transfer is claimed. Prior runtime label controls keep their original subject and unchanged fixture identities; no new runtime-label replay or central library suite is claimed here.

All commands have terminated. This intermediate remains a release hold for the new getter failure and explicit usefulness/adjudication costs. All five selection rows, hard-cut inactivity and public-promotion trust holds remain unchanged. No integration branch, baseline, label, expectation, security setting, CI dispatch or release was changed. The prior Actions proxy403 was not retried or bypassed.
