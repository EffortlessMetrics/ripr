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
