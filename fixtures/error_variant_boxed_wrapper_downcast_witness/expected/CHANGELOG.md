# Golden Output Changes

## Pending — error_variant_boxed_wrapper_downcast_witness (1)

Reason:
RIPR-SPEC-0106: issue #3700 producer regression fixture pinning boxed-error wrapper downcast witness credit and fail-closed companions

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (2)

Reason:
RIPR-SPEC-0106: issue #3700 producer regression fixture pinning boxed-error wrapper downcast witness credit and fail-closed companions

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (3)

Reason:
RIPR-SPEC-0106: re-bless after #3700 wrapper-seam fail-open gate — wrong-sibling and unrelated-enum witness sites corrected from exposed to weakly_exposed

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (4)

Reason:
RIPR-SPEC-0106: re-hash after stripping trailing whitespace from diff.patch blank context lines for git diff --check; parser treats blank and space-only context identically, classifications unchanged (2 exposed, 10 weak)

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (5)

Reason:
RIPR-SPEC-0106: re-hash after adding Display and Error impls so the fixture input compiles standalone like its siblings; impls sit outside all diff hunks, classifications unchanged (2 exposed, 10 weak)

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (6)

Reason:
RIPR-SPEC-0106: #3700 round-2 review (coderabbit g262-) — wrapper-seam missing-discriminator text and advice now name the wrapper-to-variant binding instead of generic exact-variant advice; classifications unchanged (2 exposed, 10 weak)

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (7)

Reason:
RIPR-SPEC-0106: #3700 final consolidation — wrapper map_err seams emit the typed wrapper_error_binding_unresolved limitation instead of lexical binding credit; classifications unchanged except the wrapper sites drop below exposed

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (8)

Reason:
RIPR-SPEC-0106: normalize empty context lines in the fixture diff.patch for git diff --check hygiene; classifications unchanged

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (9)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (10)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (11)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (12)

Reason:
RIPR-SPEC-0122: exposed findings default to info severity

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (13)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (14)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (15)

Reason:
RIPR-SPEC-0122: #4321 additive per-finding id lines in human-full (drill-in identifiers)

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
## Pending — error_variant_boxed_wrapper_downcast_witness (15)

Reason:
RIPR-SPEC-0122: bounded human surfaces disclose their windows - related-test and observed-value caps, digest missing-discriminator and related-test totals, Hidden block names omitted findings by file:line (class) with the all-base-side distinction (#4320); RIPR-SPEC-0152: all-base-side runs name base-side evidence instead of a lower-priority framing

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (16)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/human.txt`
## Pending — error_variant_boxed_wrapper_downcast_witness (15)

Reason:
RIPR-SPEC-0122: #4321 additive per-finding id lines in human-full (drill-in identifiers)

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_downcast_witness (17)

Reason:
RIPR-SPEC-0122: #4322 summary header names all seven classes with canonical tokens and a shown/total denominator

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
## Pending — error_variant_boxed_wrapper_downcast_witness (18)

Reason:
RIPR-SPEC-0001: quarantine unresolved property macro promotion and honor producer-owned typed limitations in human triage; ordinary discriminator controls remain unchanged

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_downcast_witness --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
