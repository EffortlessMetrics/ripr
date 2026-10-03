# Golden Output Changes

## Pending — rust_proptest_quickcheck_tests (1)

Reason:
RIPR-SPEC-0001: index proptest! and quickcheck! tests so the fixture crate relates gate_threshold

Command:
`cargo xtask goldens bless rust_proptest_quickcheck_tests --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending — rust_proptest_quickcheck_tests (2)

Reason:
RIPR-SPEC-0001: quarantine unresolved property macro promotion and honor producer-owned typed limitations in human triage; ordinary discriminator controls remain unchanged

Command:
`cargo xtask goldens bless rust_proptest_quickcheck_tests --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## #5051 review correction

Opaque property-only call arguments and lexical fallback bodies provide no reach, infection or propagation proof. Known unrelated-package mentions cannot suppress a real gap.
