# Golden Output Changes

## Pending — guarded_result_match_conditional_failure (1)

Reason:
RIPR-SPEC-0175: #3731 review fix F4 — bounded depth-0 terminal grammar; new fixture pins that conditional panics, unrelated unwraps, and closure-nested markers never credit (no_static_path)

Command:
`cargo xtask goldens bless guarded_result_match_conditional_failure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — guarded_result_match_conditional_failure (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless guarded_result_match_conditional_failure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — guarded_result_match_conditional_failure (3)

Reason:
RIPR-SPEC-0082/RIPR-SPEC-0122 wording owner change (PR #3978): Why lines re-derived from reach/observe stage state, preview notes use language display names with singular file counts, recovery detail lines end with exactly one period; mechanical re-render of unchanged fixture evidence

Command:
`cargo xtask goldens bless guarded_result_match_conditional_failure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — guarded_result_match_conditional_failure (4)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless guarded_result_match_conditional_failure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — guarded_result_match_conditional_failure (5)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless guarded_result_match_conditional_failure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — guarded_result_match_conditional_failure (6)

Reason:
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless guarded_result_match_conditional_failure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — guarded_result_match_conditional_failure (7)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless guarded_result_match_conditional_failure --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — guarded_result_match_conditional_failure (8)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless guarded_result_match_conditional_failure --reason "..."`

Updated:
- `expected/human.txt`
