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
RIPR-SPEC-0122: Python pytest verify commands now emit python -m pytest so a flat-layout package imports from the repository root; wording only, no exposure class change

Command:
`cargo xtask goldens bless python_src_layout_package_import --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — python_src_layout_package_import (8)

Reason:
RIPR-SPEC-0122: Hidden remainder names omitted preview-language identity (#4395); formatting-only, no class or JSON change.

Command:
`cargo xtask goldens bless python_src_layout_package_import --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
