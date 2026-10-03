# Golden Output Changes

## Pending — predicate_boundary_oracle_refused (1)

Reason:
RIPR-SPEC-0197/#5027 and SPEC0186: a refused boundary equality cannot borrow an admitted far oracle; retain far strength/observation but require weak discrimination. Exact compiled mutant controls pass both tests.

Command:
`cargo xtask goldens bless predicate_boundary_oracle_refused --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
