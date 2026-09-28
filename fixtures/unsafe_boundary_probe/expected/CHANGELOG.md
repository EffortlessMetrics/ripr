# Golden Output Changes

## Pending — unsafe_boundary_probe (1)

Reason:
RIPR-SPEC-0168: new fixture pinning the unsafe_boundary static_unknown probe at a changed interior line

Command:
`cargo xtask goldens bless unsafe_boundary_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unsafe_boundary_probe (2)

Reason:
RIPR-SPEC-0168: multi-hunk redesign — ordinary probes beside the boundary probe and controls that enter the diff

Command:
`cargo xtask goldens bless unsafe_boundary_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unsafe_boundary_probe (3)

Reason:
RIPR-SPEC-0168: composition of PR witness integration with main #3579 facts/owner resolution — witness path (unchanged by merge) now completes for the field_construction probe, upgrading propagate stage to the same Complete-witness wording already pinned by 15 sibling fixtures; finding classification and confidence unchanged

Command:
`cargo xtask goldens bless unsafe_boundary_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unsafe_boundary_probe (4)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless unsafe_boundary_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unsafe_boundary_probe (5)

Reason:
RIPR-SPEC-0082/RIPR-SPEC-0122 wording owner change (PR #3978): Why lines re-derived from reach/observe stage state, preview notes use language display names with singular file counts, recovery detail lines end with exactly one period; mechanical re-render of unchanged fixture evidence

Command:
`cargo xtask goldens bless unsafe_boundary_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unsafe_boundary_probe (6)

Reason:
RIPR-SPEC-0001: a changed line inside a function no test reaches is no_static_path whatever its probe shape; static_unknown escalate-to-mutation advice no longer stands in for a missing test
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless unsafe_boundary_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unsafe_boundary_probe (7)

Reason:
RIPR-SPEC-0122: unreached static_unknown next step hedges macro and integration reach

Command:
`cargo xtask goldens bless unsafe_boundary_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unsafe_boundary_probe (8)

Reason:
RIPR-SPEC-0094: re-bless after merging main (#4425 observed-value caps); unreached-owner findings read no_static_path and new-function signature lines are not probed (#4428)

Command:
`cargo xtask goldens bless unsafe_boundary_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
