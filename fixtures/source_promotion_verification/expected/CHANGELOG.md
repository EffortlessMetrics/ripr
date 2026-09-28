# Golden Output Changes

## Pending

Reason:
Align new source-promotion verifier fixture with its intentional exact-join validation input

Command:
`cargo xtask goldens bless source_promotion_verification --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0122: the source/W7 join adopts the frozen W7 bounded human check renderer and RIPR-SPEC-0147 parser-shape probe canonicalization, so this source-authored fixture records the combined-tree analyzer output rather than the source-parent renderer output

Command:
`cargo xtask goldens bless source_promotion_verification --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — source_promotion_verification (2)

Reason:
RIPR-SPEC-0149: J trial join regeneration; swarm #4002 omits the default base under --diff, swarm no_static_path why-text; findings unchanged

Command:
`cargo xtask goldens bless source_promotion_verification --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — source_promotion_verification (3)

Reason:
J: swarm #4264 stops seeding probes on brace-only lines (RIPR-SPEC-0001); this source-only fixture loses its closing-brace static_unknown finding

Command:
`cargo xtask goldens bless source_promotion_verification --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — source_promotion_verification (4)

Reason:
RIPR-SPEC-0122: J cutoff join regeneration; swarm #4379 drill-in lines and digest why-line text plus swarm unreached static_unknown next step applied to ripr-side fixtures; classes and counts unchanged

Command:
`cargo xtask goldens bless source_promotion_verification --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
