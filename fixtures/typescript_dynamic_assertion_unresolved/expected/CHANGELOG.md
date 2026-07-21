# Golden Output Changes

## Pending

Reason:
RIPR-SPEC-0085 §PR5: new fixture for typescript_dynamic_assertion_unresolved limitation.
The test uses `expect(clamp(-5, 0, 10)).toBe(expectedMin)` where `expectedMin` is a local
variable (non-literal dynamic argument). The adapter emits the
`typescript_dynamic_assertion_unresolved` named limitation and the
`typescript_oracle_observed`/`typescript_oracle_confidence`/`typescript_oracle_evidence_ref`
metadata lines. No `typescript_oracle_expected` is emitted because the argument is dynamic.

Command:
`cargo xtask goldens bless typescript_dynamic_assertion_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0088 §PR8: named limitation now surfaced for blocked TS packet

Command:
`cargo xtask goldens bless typescript_dynamic_assertion_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless typescript_dynamic_assertion_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless typescript_dynamic_assertion_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless typescript_dynamic_assertion_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0094: additive relation_reason+relation_confidence fields now populated in TS related_tests (DirectOwnerCall->direct_owner_call/high, ImportedOwnerCall->import_path_affinity/medium, etc.); existing fixture classes, oracle strength, and probe data unchanged

Command:
`cargo xtask goldens bless typescript_dynamic_assertion_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless typescript_dynamic_assertion_unresolved --reason "..."`

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
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless typescript_dynamic_assertion_unresolved --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
- `expected/human-full.txt`
