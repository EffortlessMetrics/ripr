# Post-#2933 FieldConstruction route-yield observation

This is the analysis-only rerun of the three authorized FieldConstruction
pilot heads owned by [#3074](https://github.com/EffortlessMetrics/ripr-swarm/issues/3074).
One current RIPR binary analyzed each exact recorded repository/base/head.
No consumer production source was edited, pushed, or counted as an eligible
repair attempt.

Analyzer identity:

- source `ade6998adbdd8c719a4108f43c9dfec93cb245a1` (contains landed #2933);
- `ripr 0.11.0 (ade6998adbdd8c719a4108f43c9dfec93cb245a1)`;
- binary SHA-256 `c49f448ea03f7356c6a4b4a477496f8f830b43e0545a351a5318563bb6b4f713`;
- rustc `1.95.0 (59807616e 2026-04-14)`.

The ordinary documented command was `ripr review-comments --root . --base
<sha> --head <sha>` with the default 120000 ms cooperative budget. Compact
per-candidate receipts, card extracts, and artifact digests are in
[`metrics/rust-repair-trust/3074-post-2933-observations.json`](../../metrics/rust-repair-trust/3074-post-2933-observations.json).

## Results

| Candidate | Exact base / head | Native observation | Earliest incomplete stage | Corpus treatment | Next authority |
| --- | --- | --- | --- | --- | --- |
| ripr-swarm #1580 | `91a720f390cb96c2e6c52d08e4215f2cb30942a3` / `2f71673f9c2da4532a1c1fb8e011dc6fd586ad2f` | Exit 0 in 8.551s; receipt `complete`; `limited_diff_scope`. Two inline FieldConstruction cards name producer-owned `field_value`; `gap_state` remains `static_limitation`; related_test is null. | `missing_discriminator_evidence` | identity-matched duplicate of `ripr-1580-static-limitation`; reason stays `static_limitation_no_repair_packet` | #1601 / #1981 |
| ub-review #772 | `9838259a704a5cf3748eb81af29536b99bf7cf3b` / `217633ca232120a021c7dc975973abdcb5056d39` | Exit 0 in 12.936s; receipt `complete`; `limited_diff_scope`. Zero inline cards. Ten summary-only FieldConstruction cards name `field_value` on unchanged same-file production literals. Exact Rust diff is a `cfg(test)` workflow-contract assertion in `src/main.rs`. | `no_current_behavior_change` | identity-matched duplicate of `ub-772-static-limitation`; exclusion reason updated in place | #3213 / #3160 |
| ub-review #744 | `01204a3c9a82a40b072b31495ffd612c36dd5a40` / `84a365e4f509c866e01b51cbd5e5ae0c22b7302b` | Exit 0 in 5.229s; receipt `complete`; `limited_diff_scope`. Seven summary-only FieldConstruction cards name `field_value`. Inline cards are operand-family and error-variant limitations. | `missing_discriminator_evidence` | identity-matched duplicate of `ub-744-static-limitation`; reason stays `static_limitation_no_repair_packet` | #1601 / #1981 |

All three consumer tracked-change counts remained zero. No candidate produced an
actionable card or a canonical complete route. Timeout is not the current
terminal state for these heads on this binary.

## Counting boundary

The rerun adds three observed runs, zero new exclusions, zero cases, and zero
eligible attempts. Unique exclusions stay 24. The prior ub-review #747
duplicate remains one of four duplicate observations.

Wrong-owner, sibling-field, broad-oracle, and token-coincidence controls remain
the fixture-owned surfaces under RIPR-SPEC-0125 and the evidence-promotion
honesty corpus. They were not treated as repair packets.

## Claim boundary

This observation establishes post-#2933 real-repository route yield for the
three governed FieldConstruction heads. It does not establish repair
usefulness, support promotion, or that naming `field_value` closes a gap.
Repair execution remains #3075. Corpus growth remains #1560. Boundary-operand
work remains #1581 / #1660 / #1679.
