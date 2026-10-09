# Fixture: predicate_boundary_oracle_admitted

Spec: RIPR-SPEC-0197

Owner: analysis-fixtures

Issue: #5027

## Given

The changed predicate uses `amount >= discount_threshold`.
One test directly executes exact boundary equality; a second test
executes an exact equality for the non-boundary input 101/100.
Both tests compile and execute. Restoring `>` fails the boundary test while the far test passes.

## When

`cargo xtask fixtures predicate_boundary_oracle_admitted` runs the public diff analyzer.
`cargo test -p ripr --test owner_pin_execution` compiles matched correct/wrong
controls, including separate-test and same-test layouts.

## Then

Exactly one predicate finding reads `exposed`.
The admitted far oracle retains strong kind/strength and Observe=yes.
Discriminate is `yes`.
The shared execution admission is required before boundary pairing can use
an equality, just as it is before reveal can credit that equality.

## Must Not

- Borrow a refused assertion's boundary subject while taking strength elsewhere.
- Erase a real admitted far oracle to obtain a non-exposed result.
- Admit unpolled assertions, change original cardinality, or claim runtime adequacy.
