# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0097: new fixture for exact toThrow payload oracle upgrade

Command:
`cargo xtask goldens bless typescript_tothrow_exact_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0104: predicate-boundary seam observed only by a toThrow error oracle now correctly weakly_exposed/missing_target_shape (error oracle does not discriminate the predicate change; needs a whitespace-input test). Corrects a latent oracle cross-talk fake-clean from the #1234 golden; oracle_kind stays exact_error_variant.

Command:
`cargo xtask goldens bless typescript_tothrow_exact_oracle --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
