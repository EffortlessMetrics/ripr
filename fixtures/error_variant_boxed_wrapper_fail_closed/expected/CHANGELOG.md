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

## Pending — error_variant_boxed_wrapper_fail_closed (6)

Reason:
RIPR-SPEC-0084: CheckInput default base is now None (was origin/main); --diff fixture envelopes honestly omit the inapplicable top-level base and record base_revision null. Only base/base_revision changed; findings, counts, and input_identity byte-identical.

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_fail_closed (7)

Reason:
RIPR-SPEC-0122: human-full carries per-finding drill-in commands (#4379); digest why-line names the incomplete stage; unreached static_unknown asks for a test first

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_fail_closed (8)

Reason:
RIPR-SPEC-0122: omit zero-count languages, keep the empty-result caveat to empty runs, cut digest lines at word boundaries

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_fail_closed (9)

Reason:
RIPR-SPEC-0122: digest Next step wraps instead of cutting the remedy; stop reasons carry a gloss; boxed-wrapper limitation text has no whitespace runs (#4323)
RIPR-SPEC-0122: bounded human check output leads the exposure line with the plain word the summary uses (weak, no path, unknown) before the schema value

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_fail_closed (10)

Reason:
RIPR-SPEC-0122: the analysis outcome and state lines lead with plain words; the id stays in parentheses

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_fail_closed --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## Pending — error_variant_boxed_wrapper_fail_closed (11)

Reason:
#4324 / RIPR-SPEC-0122: the bounded digest names all five stage states on one compact Evidence line (reach, infection, propagation, observation, discriminator) instead of letting the positional 2-line detail window hide the decisive stages behind a bare count; the detail window keeps its two lines and the remainder line now reads `N more detail line(s) in --format human-full`. JSON output is unchanged.

Command:
`cargo xtask goldens bless error_variant_boxed_wrapper_fail_closed --reason "..."`

Updated:
- `expected/human.txt`
