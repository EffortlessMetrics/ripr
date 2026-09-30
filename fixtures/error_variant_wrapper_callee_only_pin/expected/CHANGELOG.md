# Golden Output Changes

## Pending — error_variant_wrapper_callee_only_pin (1)

Reason:
RIPR-SPEC-0106: new #3700 BUG-1 negative fixture, callee-only pin stays weakly_exposed

Command:
`cargo xtask goldens bless error_variant_wrapper_callee_only_pin --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_variant_wrapper_callee_only_pin (2)

Reason:
RIPR-SPEC-0106: #3700 round-2 review (coderabbit g262-) — wrapper-seam advice now names the wrapper-to-variant binding instead of generic exact-variant advice; classifications unchanged (weakly_exposed)

Command:
`cargo xtask goldens bless error_variant_wrapper_callee_only_pin --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_variant_wrapper_callee_only_pin (3)

Reason:
RIPR-SPEC-0106: #3700 final consolidation — wrapper map_err seams emit the typed wrapper_error_binding_unresolved limitation instead of lexical binding credit; classifications unchanged except the wrapper sites drop below exposed

Command:
`cargo xtask goldens bless error_variant_wrapper_callee_only_pin --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_variant_wrapper_callee_only_pin (4)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless error_variant_wrapper_callee_only_pin --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_variant_wrapper_callee_only_pin (5)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless error_variant_wrapper_callee_only_pin --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_variant_wrapper_callee_only_pin (6)

Reason:
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless error_variant_wrapper_callee_only_pin --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_variant_wrapper_callee_only_pin (7)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless error_variant_wrapper_callee_only_pin --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — error_variant_wrapper_callee_only_pin (8)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless error_variant_wrapper_callee_only_pin --reason "..."`

Updated:
- `expected/human.txt`
