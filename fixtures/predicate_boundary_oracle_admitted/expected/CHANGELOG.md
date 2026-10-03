# Golden Output Changes

## Pending — predicate_boundary_oracle_admitted (1)

Reason:
RIPR-SPEC-0197/#5027 and SPEC0186: matched direct boundary positive remains exposed and fails the compiled boundary mutant; separate far assertion retains independent meaning.

Command:
`cargo xtask goldens bless predicate_boundary_oracle_admitted --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
