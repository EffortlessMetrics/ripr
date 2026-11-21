# Golden Output Changes

## Pending — error_variant_boxed_wrapper_fail_closed (1)

Reason:
RIPR-SPEC-0106: #3700 round-1 review — all-weak corpus source fixture pinning wrong-sibling and unrelated-enum wrapper witnesses below exposed

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_fail_closed (2)

Reason:
RIPR-SPEC-0106: normalize empty context lines in the fixture diff.patch for git diff --check hygiene; classification unchanged

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_fail_closed (3)

Reason:
RIPR-SPEC-0106: #3700 round-2 review (coderabbit g262-) — wrapper-seam missing-discriminator text and advice now name the wrapper-to-variant binding instead of generic exact-variant advice; classifications unchanged (4 weakly_exposed)

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_fail_closed (4)

Reason:
RIPR-SPEC-0106: #3700 final consolidation — wrapper map_err seams emit the typed wrapper_error_binding_unresolved limitation instead of lexical binding credit; classifications unchanged except the wrapper sites drop below exposed

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_fail_closed (5)

Reason:
RIPR-SPEC-0106: strip trailing whitespace from diff.patch blank-context lines for git diff --check; input_identity re-hash only, classifications unchanged

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
