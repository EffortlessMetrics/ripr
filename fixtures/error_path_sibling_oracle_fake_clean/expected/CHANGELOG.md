# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0107: initial golden for error_path fake-clean repro fixture; error_path with only sibling ExactValue oracle correctly reports weakly_exposed via observation_unverified

Command:
`cargo xtask goldens bless error_path_sibling_oracle_fake_clean --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless error_path_sibling_oracle_fake_clean --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
add human-full golden for exhaustive evidence-promotion projection while default human stays bounded

Command:
cargo xtask goldens check

Updated:
- `expected/human-full.txt`

## Pending

Reason:
restrict CallDeletion probes to standalone call statements; refresh affected goldens and record intentional output changes

Command:
`cargo xtask goldens bless error_path_sibling_oracle_fake_clean --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
