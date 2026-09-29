# Golden Output Changes

## Pending — error_variant_wrapper_wrong_receiver_pin (1)

Reason:
RIPR-SPEC-0106: new #3700 wrong-receiver negative fixture, receiver-qualified owner call must not confirm

Command:
`cargo xtask goldens bless error_variant_wrapper_wrong_receiver_pin --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_variant_wrapper_wrong_receiver_pin (2)

Reason:
RIPR-SPEC-0106: #3700 round-2 review (coderabbit g262-) — wrapper-seam advice now names the wrapper-to-variant binding instead of generic exact-variant advice; classifications unchanged (weakly_exposed)

Command:
`cargo xtask goldens bless error_variant_wrapper_wrong_receiver_pin --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_variant_wrapper_wrong_receiver_pin (3)

Reason:
RIPR-SPEC-0106: #3700 final consolidation — wrapper map_err seams emit the typed wrapper_error_binding_unresolved limitation instead of lexical binding credit; classifications unchanged except the wrapper sites drop below exposed

Command:
`cargo xtask goldens bless error_variant_wrapper_wrong_receiver_pin --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_variant_wrapper_wrong_receiver_pin (4)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless error_variant_wrapper_wrong_receiver_pin --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_variant_wrapper_wrong_receiver_pin (5)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless error_variant_wrapper_wrong_receiver_pin --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_variant_wrapper_wrong_receiver_pin (6)

Reason:
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless error_variant_wrapper_wrong_receiver_pin --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
