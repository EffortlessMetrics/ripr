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


## Pending - source_promotion_verification

Reason:
The old diff named an absent xtask source file and described an unrelated
placeholder function. Align the diff with the unchanged src/lib.rs model and
its actual 39-to-40 length-boundary change. The real CLI now retains one
predicate finding with no_static_path; the old empty-result collapse and
phantom findings are rejected. This static model does not qualify the separate
graph/receipt verifier. No production source or model assertion was weakened.

Updated:
- expected/check.json
- expected/human.txt

Evidence:
Actual production FixtureWorkspace and normalization functions, retained exact
installed default-feature CLI 498331d, baseline zero subjects versus repaired
one current predicate, and all three real CLI output surfaces. This is focused
helper-path proof, not a full current-source xtask/Cargo goldens run.
