# Golden Output Changes

## Pending — rust_uncalled_owner_same_file_tests (1)

Reason:
RIPR-SPEC-0001: new fixture; an uncalled owner linked to tests only by same-file proximity has weak reach and is not exposed

Command:
`cargo xtask goldens bless rust_uncalled_owner_same_file_tests --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_uncalled_owner_same_file_tests (2)

Reason:
RIPR-SPEC-0001: fixture diff.patch drops blank context lines (git diff --check); only input_identity changes

Command:
`cargo xtask goldens bless rust_uncalled_owner_same_file_tests --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_uncalled_owner_same_file_tests (3)

Reason:
RIPR-SPEC-0084: re-emit this branch's new fixture under the landed None-default base envelope (top-level base omitted, base_revision null, summary-first ordering); findings, reach, classes and confidences unchanged

Command:
`cargo xtask goldens bless rust_uncalled_owner_same_file_tests --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_uncalled_owner_same_file_tests (4)

Reason:
RIPR-SPEC-0001: an owner no test calls, with no production caller and no opaque test macro, has reach no; same-file neighbour tests stay as suggested locations and no longer read as observing it, so the finding is no_static_path instead of weakly_exposed

Command:
`cargo xtask goldens bless rust_uncalled_owner_same_file_tests --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_uncalled_owner_same_file_tests (5)

Reason:
RIPR-SPEC-0001: neighbour tests that never call the owner no longer read as activating it either, so infection is no and the headline score drops with it

Command:
`cargo xtask goldens bless rust_uncalled_owner_same_file_tests --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_uncalled_owner_same_file_tests (6)

Reason:
RIPR-SPEC-0001: unreached-stage wording says no test can activate, observe or discriminate the change
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless rust_uncalled_owner_same_file_tests --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_uncalled_owner_same_file_tests (7)

Reason:
RIPR-SPEC-0094: re-bless after merging main (#4425 observed-value caps); unreached-owner findings read no_static_path and new-function signature lines are not probed (#4428)

Command:
`cargo xtask goldens bless rust_uncalled_owner_same_file_tests --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_uncalled_owner_same_file_tests (8)

Reason:
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless rust_uncalled_owner_same_file_tests --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_uncalled_owner_same_file_tests (9)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless rust_uncalled_owner_same_file_tests --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
