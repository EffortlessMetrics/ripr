# Golden Output Changes

## Pending — unsafe_boundary_probe (1)

Reason:
RIPR-SPEC-0168: new fixture pinning the unsafe_boundary static_unknown probe at a changed interior line

Command:
`cargo xtask goldens bless unsafe_boundary_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unsafe_boundary_probe (2)

Reason:
RIPR-SPEC-0168: multi-hunk redesign — ordinary probes beside the boundary probe and controls that enter the diff

Command:
`cargo xtask goldens bless unsafe_boundary_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — unsafe_boundary_probe (3)

Reason:
RIPR-SPEC-0168: composition of PR witness integration with main #3579 facts/owner resolution — witness path (unchanged by merge) now completes for the field_construction probe, upgrading propagate stage to the same Complete-witness wording already pinned by 15 sibling fixtures; finding classification and confidence unchanged

Command:
`cargo xtask goldens bless unsafe_boundary_probe --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
