# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0154: initial parity fixture (#3284)

Command:
`cargo xtask goldens bless assertion_form_parity_err_guard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0154: initial parity fixtures — identical oracle, classification, and gap accounting across equivalent harness forms (#3284)

Command:
`cargo xtask goldens bless assertion_form_parity_err_guard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0158: the additive per-value provenance field now surfaces the evaluation chains and call sources the line-keyed assertion_texts map dropped (deferred #3295 follow-up)

Command:
`cargo xtask goldens bless assertion_form_parity_err_guard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless assertion_form_parity_err_guard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — assertion_form_parity_err_guard (1)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless assertion_form_parity_err_guard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — assertion_form_parity_err_guard (2)

Reason:
RIPR-SPEC-0001 (#4216 row 5): brace-only and else-only changed lines (`}`, `} else {`) no longer seed static_unknown probes. Only those findings are removed; every remaining finding is byte-identical, and summary/outcome counts drop by the removed count.

Command:
`cargo xtask goldens bless assertion_form_parity_err_guard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — assertion_form_parity_err_guard (3)

Reason:
RIPR-SPEC-0001: the one-line signature of a new function whose body is added too carries no behavior of its own and is no longer probed (parity with the TypeScript and Python adapters)

Command:
`cargo xtask goldens bless assertion_form_parity_err_guard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — assertion_form_parity_err_guard (4)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless assertion_form_parity_err_guard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — assertion_form_parity_err_guard (5)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless assertion_form_parity_err_guard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — assertion_form_parity_err_guard (6)

Reason:
RIPR-SPEC-0122: bounded human surfaces disclose their windows - related-test and observed-value caps, digest missing-discriminator and related-test totals, Hidden block names omitted findings by file:line (class) with the all-base-side distinction (#4320); RIPR-SPEC-0152: all-base-side runs name base-side evidence instead of a lower-priority framing

Command:
`cargo xtask goldens bless assertion_form_parity_err_guard --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — assertion_form_parity_err_guard (7)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless assertion_form_parity_err_guard --reason "..."`

Updated:
- `expected/human.txt`
