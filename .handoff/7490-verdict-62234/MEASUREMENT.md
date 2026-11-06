# RIPR #7490 provisional measurement packet — release hold

## Subject and custody

Provisional commit **62234ac13b0f7a8ff7e30d239f59215b808775b3**, tree **ef7221a5c23a73670d23bc9998cfe1496b4e38f0**; parent frozen input b51aebc7e4735529befd677ebf14071c7f09e477, tree d3142dd59d4dcfea5e6b4981732f61c8516f3fa0. The exact delta is eight spec-scope TOMLs in spec-scope-repair.patch. `crates` trees both equal 78c461c7248ab59d77f1381f68ec725e56ed2d17; `xtask` trees both equal fc6b11110c240c38f101ceb4ba687ae2eb9d31b9. All **1,698 scoring definitions are unchanged** from b51. See product-tree-parity.json and b51-definition-parity.json.

The metadata repair is substantively reviewed in REVIEW.md. It records the two existing prose-only specs as explicitly unmeasured; it does not alter or waive their packaging/promotion obligations. b51's failed pre-analysis validation remains retained in the predecessor packet. None of the following executions is described as a b51 run or selected-cut qualification.

Linux x86_64, Bash; rustc 1.95.0 (59807616e 2026-04-14), cargo 1.95.0 (f2d3ce0bd 2026-03-21); dev build, one Cargo job, no incremental compilation, one Rayon thread. The default and Perl actors both report `ripr 0.11.0 (62234ac13b0f7a8ff7e30d239f59215b808775b3)`. Actors were copied, not hard-linked, into inactive retained witness slots and verified against the source bytes before/after copying. actors.json retains paths, sizes, hashes and versions. CLI SHA256: **84b87f91226965dbc4e91c7b09d880c0398b4154e474e3a7baec6d254ff8f0c4**; Perl actor **832ecf745ce0630d296ab115250d681dacf1bfb352b5061dcfcf9c7b984a60d8**; xtask **c5e1a4539a7a88f5d096de477fb932095b7597a4d4a0ee40cfcea84c9359efaa**. Executable witnesses remain local; this is evidence transport, not a transfer of package qualification.

Corpus Git-object inventory SHA256 **40bebd79b5bf5e3587fa8653d1fc344c88e1ceb8f0daffa7d75bfe16bc481cad**; case-definition inputs and hashes are in definitions.json. Source stayed clean at the provisional commit throughout analysis and was read back at completion.

## Full live results and repeat

`cargo build --locked -p xtask` passed. Full `xtask verdict-corpus check-all` executed all four populations and exited **1**, correctly rejecting the new Rust false rows and changed expected observations. Expectations, labels, thresholds, baselines and ledgers were not blessed or edited.

| Language | Cases | False verdicts | False actionable | False exposed / false silent | Abstained | Missed credit / discriminated truth |
|---|---:|---:|---:|---:|---:|---:|
| Rust | 846 | 24/846 | 24/433 | 0/413 each | 328/846 | 172/433 |
| TypeScript | 373 | 0/373 | 0/212 | 0/161 each | 50/373 | 19/212 |
| Python | 385 | 0/385 | 0/181 | 0/204 each | 83/385 | 12/181 |
| Perl | 94 | 0/94 | 0/36 | 0/58 each | 49/94 | 30/36 |
| Total | 1,698 | 24/1,698 | 24/862 | 0/836 each | 510/1,698 | 233/862 |

Ideal outcomes total 1,164. Every case was selected, executed and classified; no case was skipped, removed or declared ineligible. Case-selected subjects total **399**; retained inventory has **400** because Perl's authored-perl-testmore-pathparts subject is currently referenced by no case (not a skipped case). Each language's complete report is in full/. Full independent `report --language <language> --out <owned repeat>` executions all exit 0 and their complete JSON reports, including every row and coverage field, are semantically equal to the first reports; stable_repeat=true for all four in packet.json. Report mode does not run the false-row/expected-drift check gate, so exit 0 is stability evidence, not a passing quality gate.

Contradictions are 0/1035 Rust row/probe pairs, 0/373 TypeScript, 0/385 Python and 0/94 Perl. Rust oracle-kind accuracy is 49/49, overclaim 0/49. Spec example coverage is Rust 330/330, TS 162/164, Python 62/62, Perl 32/33, with the existing floors unchanged. Both specs 0374/0375 are visibly listed as unmeasured in all four emitted reports.

Origin is a separate axis: Rust has 805 authored cases (24 false, 291 abstained) and 41 upstream-origin cases (0 false, 37 abstained, four ideal). The other three corpora are authored. packet.json retains every origin slice and Perl producer/edited-producer slice. These figures are not real-world accuracy estimates.

## Comparable populations

All four populations have exactly the same definitions as main-tip **984c69a290486b1da9d91700a6f771045ab079e1**: Rust 846, TS 373, Python 385, Perl 94 unchanged; zero added/removed/changed. Against that tree's retained expected observations, Rust changes **0→24 false actionable**, **321→328 abstained**: exactly 24 ideal→false-actionable and seven ideal→abstained. Other language outcomes are unchanged. The prior executed Windows receipt remains source/host-bound historical evidence; that historical executable was not freshly rerun here.

| Older Rust source | Unchanged definitions | Changed | Added | Removed | Same-definition false verdicts, before→current | Same-definition abstention, before→current |
|---|---:|---:|---:|---:|---:|---:|
| 3855533a7e8ecdfcd075b5895aa334fc10863466 | 448 | 29 | 369 | 0 | 27/448→11/448 | 169/448→147/448 |
| 806147b2184d1ff703a883abf68215228a7d2bef | 460 | 29 | 357 | 0 | 9/460→11/460 | 166/460→147/460 |

The older sides above are committed observations, not fresh historical executions. Changed definitions and added cases remain separately scored in packet.json; longer-term improvement cannot hide the regression against the recent 846-case population. Original-203 identity checks retain **199 unchanged definitions and four changed Serde definitions**. Current unchanged-199 figures: false 3/199 (FA3/102, FE0/97, FS0/97), abstention64/199. The changed four all abstain; all-original-203 current figures are false3/203 and abstention68/203. The legacy #7552 comparator test fixture's fractions are separately recorded in original-baseline-fractions.json as fixture input, not substituted for the issue's separately sourced historical 63/203 narrative or a verified historical executable run.

## Raw execution, integrity and controls

Independent direct-CLI capture retained **1,698/1,698 native exit-0 outputs**, every argv, byte count and stdout/stderr digest. The initial collector exited 1 because it looked for the nested outcome at the wrong JSON level. Initial receipts/summary are preserved; reparse_raw.py verified the existing byte hashes, exact diff input identities, single expected-language routing and affirmative language completeness, then recovered metadata without rerunning analysis. This instrument failure is not called a product command failure or erased from commands.json.

Raw outcomes: Rust 832 complete_with_findings, 13 legitimate no_behavioral_candidates, one partial_with_limitations; TS 309 complete, 64 partial; Python 385 complete; Perl two complete, 92 partial. Partial analysis and zero candidates remain explicit and do not become definitive exposure verdicts.

- Existing repository integrity tests: **32 passed, zero failed**, 5,390 filtered; no full library suite. They include missing/stale expected-language evidence, population comparability, dropped/missing rows and scope-file law.
- Analytical packet-custody audit: truthful failure packet accepted; all six injections rejected (dropped case, changed expectation/definition, stale candidate, zero execution, mismatched corpus identity, and abstention substituted for repaired credit). This audit is not a new classifier or installed release gate.
- Existing runtime relabel producer, repeat **2**: four known controls plus nine regression/sibling controls, **13 cases / 100 test-command observations / zero label drift**. Positive baseline/edited test runs pass, labeled wrong mutants fail for the discriminated cases, and unreached/swallowed/no-observer negatives remain not discriminated. Every source label and expected verdict remains unchanged. Other 1,685 labels were not freshly replayed.

## Shared failures and readiness

clusters.json and shared-authority-evidence.json separate **17 identity_unresolved**, **four observation_unverified**, and **three same_test_pairing_missing** failures. CLUSTERS.md describes the production authority and executable controls. Three direct CLI reproductions are retained separately. Do not assume a single identity-gate repair fixes all 24.

Seven additional missed-credit rows are named separately in denominators-and-missed-credit.json. Parsed ledger entry counts are Rust165, TS19, Python12, Perl30; the seven new Rust abstentions are unaccepted additions, not an invented zero or automatically repaired discrimination. No new ledger entries were created.

`xtask dx-scoreboard --boards trust --ripr-bin <candidate binary> --baseline metrics/dx-scoreboard/baseline.json --gate` exits **1**: all five Perl projections have no eligible unchanged common cases against the operational baseline. No baseline refresh or margin change. Its hostile-input sample passes **39/39**; seven metrics remain not measured and self_contradictions remains uncomparable. The scoreboard's verdict rates read committed expected rows, not this fresh live analyzer measurement; any recorded zero cannot override the actual 24 false verdicts.

**Disposition: unresolved_release_hold.** Central owner must repair/disposition the 24 wrong-advice rows, retain the seven missed-credit costs and resolve the operational comparison gap under existing #2766/#9619 authority. Source repair stays with the integration/#12010 owner; this lane wrote no analyzer repair. #11394/#11244 residual adjudication remains independent. All five selection rows stay pending, hard-cut inactive, public #1893/draft #1889 trust holds intact. No merge, remote CI, source promotion, deployment or release occurred.

Commands, guards, native results, raw data, repeat reports, proof controls and immutable source bundle are included in the evidence archive. Frozen actors and warm build state remain locally retained for repair verification; no byte witness was hard-linked into mutable output. All owned verification sessions terminated. The resource ceiling was not raised; inactive frozen copies' page cache was released without changing bytes or settings. This packet transports evidence and a metadata patch, not qualification or shipping authorization.
