# Golden Output Changes

## Pending

Reason:
new fixture: raise change observed only by a normal-path value oracle is not exposed (#1290 Class C)

Command:
`cargo xtask goldens bless python_adversarial_error_path_untaken_branch --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless python_adversarial_error_path_untaken_branch --reason "..."`

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
