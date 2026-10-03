# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0153: initial bench-evidence fixture (#3283)

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0160: the additive git_candidate_subject identity field (null for ordinary runs) in the check JSON identity block

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — benches_harness_evidence (1)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — benches_harness_evidence (2)

Reason:
RIPR-SPEC-0082/RIPR-SPEC-0122 wording owner change (PR #3978): Why lines re-derived from reach/observe stage state, preview notes use language display names with singular file counts, recovery detail lines end with exactly one period; mechanical re-render of unchanged fixture evidence

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — benches_harness_evidence (3)

Reason:
RIPR-SPEC-0001 (#4216 row 5): brace-only and else-only changed lines (`}`, `} else {`) no longer seed static_unknown probes. Only those findings are removed; every remaining finding is byte-identical, and summary/outcome counts drop by the removed count.

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — benches_harness_evidence (4)

Reason:
RIPR-SPEC-0001: a changed line inside a function no test reaches is no_static_path whatever its probe shape; static_unknown escalate-to-mutation advice no longer stands in for a missing test
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — benches_harness_evidence (5)

Reason:
RIPR-SPEC-0001: the one-line signature of a new function whose body is added too carries no behavior of its own and is no longer probed (parity with the TypeScript and Python adapters)
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — benches_harness_evidence (6)

Reason:
RIPR-SPEC-0122: unreached static_unknown next step hedges macro and integration reach

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — benches_harness_evidence (7)

Reason:
RIPR-SPEC-0094: re-bless after merging main (#4425 observed-value caps); unreached-owner findings read no_static_path and new-function signature lines are not probed (#4428)

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — benches_harness_evidence (8)

Reason:
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — benches_harness_evidence (9)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — benches_harness_evidence (10)

Reason:
RIPR-SPEC-0122: bounded human surfaces disclose their windows - related-test and observed-value caps, digest missing-discriminator and related-test totals, Hidden block names omitted findings by file:line (class) with the all-base-side distinction (#4320); RIPR-SPEC-0152: all-base-side runs name base-side evidence instead of a lower-priority framing

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — benches_harness_evidence (11)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/human.txt`

## Pending — benches_harness_evidence (12)

Reason:
RIPR-SPEC-0122: #4322 summary header names all seven classes with canonical tokens and a shown/total denominator

Command:
`cargo xtask goldens bless benches_harness_evidence --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## #5051 plain no-path guidance

Only the selected untyped no-path safe-action sentence changes. Static-limited state, selection, classification, full output and JSON remain unchanged.
