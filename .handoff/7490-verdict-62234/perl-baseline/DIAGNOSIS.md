# Five Perl comparisons: valid refusal after a producer/corpus migration

Subject inspected: provisional 62234ac13b0f7a8ff7e30d239f59215b808775b3, tree ef7221a5c23a73670d23bc9998cfe1496b4e38f0. Its production and xtask subtrees equal frozen b51 exactly. This follow-up ran no corpus, Perl fact producer, analyzer measurement, Cargo build, full library suite, CI or release action. It changed no repository file or baseline. The prior full native trust report and immutable Git objects supply the observations.

**Conclusion: no tooling/custody repair is indicated.** The five comparison failures are the documented, tested response to zero unchanged scoring definitions after reviewed packet/producer re-pins. Existing #10809 owns baseline migration; the current record explicitly leaves re-admission and the abstention judgment pending.

## Source custody and denominator identity

The operational baseline's top-level revision `10e56371911a4e6ef8020fbcd7e5f2692fc9b7f1` is not the Perl samples' source. Each Perl sample carries its own `population_provenance`:

| Samples | Exact recorded source | Population cases | Producer-eligible denominator | Pinned producer |
|---|---|---:|---:|---|
| Four false-verdict-family rates | ea5229fba660d284841b3954f73ca7948a88d0ea, approved #10809; carried by #11446 / 866f38843ab73cfaa63ec9b20f2d1bb48ec9d806 | 88 | 54 total; 28 discriminated / 26 not fully | 07320a46bd344d9c1a51dcb742484ef0042ef887 |
| Abstention | 76651d1cdf9242b301c9ddf28c442838ccf73c59, approved #8035 | 36 | 27 | 99458fde50b8204ecf7054779bb3670e6b5fdda5 |
| Current committed-record projections | 62234ac13b0f7a8ff7e30d239f59215b808775b3 | 94 | 60 total; 30 discriminated / 30 not fully | 43336765bfff10e02325b3f7ea2aa0ff60eff2ff |

Read-only reconstruction reproduced **all 388 recorded baseline contribution fingerprints** (4×88 + 36) from the declared historical source using the existing documented definition inputs. All 94 current hashes match the native producer's recorded hashes. Both historical corpus blob IDs and expected-row tree IDs match sample provenance exactly. All 124 historical packet byte digests and their current counterparts match the case pins. No stale blob, hash-algorithm mismatch, missing row, nondeterminism or denominator-projection defect was found.

All 88/36 old IDs remain present, with no removal or per-shared-case eligibility change. Current producer projections retain all 94 identities, including 34 edited-producer rows with 0/0 contributions. The four newer baselines have six added IDs; the older abstention baseline has 58 added IDs, of which 33 are producer-eligible. **Zero scoring definitions are unchanged in every comparison.** This is not zero selected/executed evidence and not an invented zero error rate.

## Exact drift

The schema, verdict projection, and fingerprint algorithm are unchanged. `population.rs::Records::load` binds verified packet SHA256, packet provenance and the entire `corpus.fact_producer` declaration into every Perl definition. Native comparison requires same case ID **and** the same definition hash.

Against `ea5229f`, all 88 definitions differ only inside the `perl_facts` portion: every packet SHA256 and the producer pin changed. Canonical packet JSON comparisons show:

- **43 packets change only producer metadata**: 40 producer packets and three edited-producer packets.
- **45 also change oracle data** (some also limitation/fingerprint data): 14 producer and 31 edited-producer packets.

Even the 43 metadata-only packets are intentionally different under the accepted identity contract; they cannot be silently normalized into an unchanged cohort. The other 45 are not just serialization/provenance churn. Source history includes the multi-package producer re-pin #11451 and matcher/block-eval grading re-pins #11636/#11812. The latter moves the declaration to 43336765.

Against the older abstention source, all 36 packet byte pins and producer declarations differ. Three edited-producer case definitions also change anchors/truth/expected semantics (`perl-0235-ex01-sink-unconfirmed`, `...ex13-method-receiver`, `...ex14-helper-call`); those changes pre-exist this task and remain distinct in the packet. No label or expected outcome was modified by this lane.

`population.rs::compare` therefore correctly returns `comparison_failed=true` with no unchanged common denominator. Replacing hashes by ID, dropping producer binding, treating absent denominators as zero or refreshing the baseline merely to make this pass would weaken the existing contract.

## Rates and the actual decision

These are committed-record projections from the already retained native trust report, not fresh analyzer executions in this follow-up:

| Metric | Recorded baseline | Current retained IDs, **not definition-comparable** | Added eligible cases | Current full producer cohort |
|---|---:|---:|---:|---:|
| False verdict | 1/54 | 0/54 | 0/6 | 0/60 |
| False actionable | 0/28 | 0/28 | 0/2 | 0/30 |
| False exposed | 1/26 | 0/26 | 0/4 | 0/30 |
| False silent | 0/26 | 0/26 | 0/4 | 0/30 |
| Abstention | 8/27 | 16/27 | 27/33 | 43/60 |

The older retained 27-ID transition is not eight newly proven analyzer regressions: five former false-actionable cases become limited (harmful advice removed, discrimination still unestablished); one former false-exposed case becomes limited; and two ideal cases become limited. Of the two ideal→limited cases, `perl-testmore-points-cmp-ok-positive` already has the explicit fail-closed disposition recorded on #10874. `perl-test2-close-month-subtest` is the retained missed-credit cost with the existing producer/relation owner. The three other formerly accepted #10874 abstentions now read honest gaps. `retained-abstention-transitions.json` preserves every row and truth/outcome transition.

The decision is **owner-reviewed baseline re-admission, not a code repair**: preserve this historical cohort and its costs, explicitly accept/reject the present 43/60 level and producer/corpus migration under #10809/#2766/#9619, and only then have the owning baseline lane update the specifically authorized samples/provenance and derived proof copy. Prior approval of the four false-verdict rates and four specific correct refusals did not approve an aggregate abstention reset. No fresh approval is inferred here.

If re-admission is not accepted, keep the current comparison refusal. Another evidence route is an explicit replay of original baseline inputs with the chosen analyzer, reporting the newer producer population separately; that is a different named measurement subject, not substituting new hashes into old observations. This task did not execute that route or create a new scorer. The existing producer programme is not automatically made a new release prerequisite by this diagnosis.

## Executable controls and established ownership

`diagnose.py --repo <checkout> --current-report <retained native dx-scoreboard.json> --out <owned result>` replays the read-only custody/hash/population audit. It uses source objects and retained rows and never invokes the analyzer, producer, CI or a baseline writer. `diagnosis.json` retains all IDs, fractions, provenance and the native comparison objects; native input snapshots are included in the archive.

Three existing tests were executed directly from the already compiled exact-source xtask test binary, with timeout 60s and test-threads=1: `perl_packet_and_producer_are_scoring_identity`, `missing_duplicate_and_incomparable_evidence_block_the_gate`, and `no_common_eligible_denominator_is_a_blocking_comparison_failure`. **3 passed / 0 failed / 5,419 filtered**, without a build. The producer test rejects an un-repinned byte change, changes identity after a legitimate byte re-pin, leaves description annotations inert, and changes every case identity when the producer pin changes. Exact argv, binary SHA256 and result are in commands.json/native-contract-controls.log.

Current #10809 primary records already establish this cause and pending decision:

- https://github.com/EffortlessMetrics/ripr-swarm/issues/10809#issuecomment-6650227674
- https://github.com/EffortlessMetrics/ripr-swarm/issues/10809#issuecomment-6655723624
- Historical bounded dispositions: https://github.com/EffortlessMetrics/ripr-swarm/pull/10874#issuecomment-6538957548

No duplicate defect, baseline writer or source repair was started. The existing baseline-layout proposal explicitly excludes Perl admission. The 24/7 Rust control mapping remains at the prior immutable CLUSTERS.md and source packet. Helper 01a1ae53-b481-736d-a5a5-11ff90bfc03c owns only the four confirmation plus three pairing repairs; central owns the 17 identity routes and all integration. This lane retains full-corpus measurement ownership and awaits a meaningful new repair subject before remeasurement.

All five selection rows, hard-cut inactivity, public #1893/#1889 trust holds and central candidate remain unchanged. Prior optional Actions-list readback remains proxy403 and was neither retried nor bypassed.
