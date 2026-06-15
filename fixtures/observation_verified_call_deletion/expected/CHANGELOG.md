# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0094: CallDeletion probe with result_key token_match does NOT emit observation_unverified; initial golden for the anti-over-correction proof fixture

Command:
`cargo xtask goldens bless observation_verified_call_deletion --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Re-author with strong whole-object/exact-value persisted-effect observer so the verified-effect control genuinely demonstrates exposed; pairs with reveal.rs effect_observer_confirms fix (#1216)

Command:
`cargo xtask goldens bless observation_verified_call_deletion --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
