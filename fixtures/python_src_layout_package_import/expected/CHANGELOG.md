# Golden Output Changes

## Pending — python_src_layout_package_import (1)

Reason:
RIPR-SPEC-0028: src-layout module identity; a package-name import of a src/ owner credits direct, a same-named function from another module stays orthogonal

Command:
`cargo xtask goldens bless python_src_layout_package_import --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — python_src_layout_package_import (2)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (#4002); this fixture's golden was blessed on a base before that change, so --diff envelopes now omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed.

Command:
`cargo xtask goldens bless python_src_layout_package_import --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — python_src_layout_package_import (3)

Reason:
RIPR-SPEC-0028: the fixture's only test call (bulk_discount(101)) agrees on both sides of the change and never sits on the quantity==100 boundary, so exposure honestly drops from exposed to weakly_exposed under the boundary rule.

Command:
`cargo xtask goldens bless python_src_layout_package_import --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — python_src_layout_package_import (4)

Reason:
RIPR-SPEC-0082/RIPR-SPEC-0122 wording owner change (PR #3978): Why lines re-derived from reach/observe stage state, preview notes use language display names with singular file counts, recovery detail lines end with exactly one period; mechanical re-render of unchanged fixture evidence

Command:
`cargo xtask goldens bless python_src_layout_package_import --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — python_src_layout_package_import (5)

Reason:
RIPR-SPEC-0122: #4216 row 1, the Python preview_limited safe next action names why no ripr command routes the finding instead of asking for repair-packet fields the operator cannot complete

Command:
`cargo xtask goldens bless python_src_layout_package_import --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — python_src_layout_package_import (6)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless python_src_layout_package_import --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — python_src_layout_package_import (7)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value
RIPR-SPEC-0122: Python pytest verify commands now emit python -m pytest so a flat-layout package imports from the repository root; wording only, no exposure class change

Command:
`cargo xtask goldens bless python_src_layout_package_import --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — python_src_layout_package_import (8)

Reason:
RIPR-SPEC-0046 RIPR-SPEC-0047: check JSON now carries a top-level source_subject stamp with the analysis-time content digests of the files a derived gap ledger names (#4544); no finding, classification, or human output changed.
RIPR-SPEC-0122: Hidden remainder names omitted preview-language identity (#4395); formatting-only, no class or JSON change.

Command:
`cargo xtask goldens bless python_src_layout_package_import --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — python_src_layout_package_import (9)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless python_src_layout_package_import --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — python_src_layout_package_import (10)

Reason:
RIPR-SPEC-0122: bounded human surfaces disclose their windows - related-test and observed-value caps, digest missing-discriminator and related-test totals, Hidden block names omitted findings by file:line (class) with the all-base-side distinction (#4320); RIPR-SPEC-0152: all-base-side runs name base-side evidence instead of a lower-priority framing

Command:
`cargo xtask goldens bless python_src_layout_package_import --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — python_src_layout_package_import (11)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless python_src_layout_package_import --reason "..."`

Updated:
- `expected/human.txt`
