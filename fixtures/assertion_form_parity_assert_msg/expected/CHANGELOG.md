# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0154: initial parity fixture (#3284)

Command:
`cargo xtask goldens bless assertion_form_parity_assert_msg --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0154: initial parity fixtures — identical oracle, classification, and gap accounting across equivalent harness forms (#3284)

Command:
`cargo xtask goldens bless assertion_form_parity_assert_msg --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0158: the additive per-value provenance field now surfaces the evaluation chains and call sources the line-keyed assertion_texts map dropped (deferred #3295 follow-up)

Command:
`cargo xtask goldens bless assertion_form_parity_assert_msg --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless assertion_form_parity_assert_msg --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — assertion_form_parity_assert_msg (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless assertion_form_parity_assert_msg --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — assertion_form_parity_assert_msg (3)

Reason:
RIPR-SPEC-0001 (#4216 row 5): brace-only and else-only changed lines (`}`, `} else {`) no longer seed static_unknown probes. Only those findings are removed; every remaining finding is byte-identical, and summary/outcome counts drop by the removed count.

Command:
`cargo xtask goldens bless assertion_form_parity_assert_msg --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
