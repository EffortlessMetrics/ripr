# Golden Output Changes

## Pending — property_macro_noop_quickcheck (1)

Reason:
RIPR-SPEC-0001: quarantine unresolved property macro promotion and honor producer-owned typed limitations in human triage; ordinary discriminator controls remain unchanged

Command:
`cargo xtask goldens bless property_macro_noop_quickcheck --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`

## #5051 review correction

Opaque property-only call arguments and lexical fallback bodies provide no reach, infection or propagation proof. Known unrelated-package mentions cannot suppress a real gap.
