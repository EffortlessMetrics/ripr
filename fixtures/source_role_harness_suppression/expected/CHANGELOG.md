# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0155: initial harness-suppression fixture — full plumbing vocabulary, zero production obligations, evidence retained (#3285)

Command:
`cargo xtask goldens bless source_role_harness_suppression --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0155: add the exact equality-boundary assertion alongside the Err-guard (review: equality case should not rest on the weak relational form alone)

Command:
`cargo xtask goldens bless source_role_harness_suppression --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0158: the additive per-value provenance field now surfaces the evaluation chains and call sources the line-keyed assertion_texts map dropped (deferred #3295 follow-up)

Command:
`cargo xtask goldens bless source_role_harness_suppression --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless source_role_harness_suppression --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0162: the propagation_unknown why-hint stops asserting the propagation the class marks unknown, and unknown-class limitation prose renders under the Analyzer limit label (human-only; the shared decision-layer text is unchanged)

Command:
`cargo xtask goldens bless source_role_harness_suppression --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — source_role_harness_suppression (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless source_role_harness_suppression --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — source_role_harness_suppression (3)

Reason:
RIPR-SPEC-0001 (#4216 row 5): brace-only and else-only changed lines (`}`, `} else {`) no longer seed static_unknown probes. Only those findings are removed; every remaining finding is byte-identical, and summary/outcome counts drop by the removed count.

Command:
`cargo xtask goldens bless source_role_harness_suppression --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
