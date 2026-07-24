# Golden Output Changes

## Pending

Reason:
gap-3 honesty-clarity reword (RIPR-SPEC-0101): jest framework known, no lockfile → limitation reworden from typescript_runner_hint_unresolved to typescript_package_manager_unresolved; verify command unchanged.

Command:
`cargo xtask goldens bless typescript_monorepo_package_local --reason "gap-3 reword: framework-known case now emits typescript_package_manager_unresolved instead of typescript_runner_hint_unresolved (RIPR-SPEC-0101)"`

Updated:
- `expected/check.json`

## Pending

Reason:
RIPR-SPEC-0085 PR6: new monorepo fixture demonstrating package-local ownership enforcement and typescript_target_unresolved limitation for cross-package test references

Command:
`cargo xtask goldens bless typescript_monorepo_package_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0088 §PR8: named limitation now surfaced for blocked TS packet

Command:
`cargo xtask goldens bless typescript_monorepo_package_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless typescript_monorepo_package_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless typescript_monorepo_package_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless typescript_monorepo_package_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0094: additive relation_reason+relation_confidence fields now populated in TS related_tests (DirectOwnerCall->direct_owner_call/high, ImportedOwnerCall->import_path_affinity/medium, etc.); existing fixture classes, oracle strength, and probe data unchanged

Command:
`cargo xtask goldens bless typescript_monorepo_package_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive typescript_test_runner evidence field (TS must-use roadmap item 3)

Command:
`cargo xtask goldens bless typescript_monorepo_package_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless typescript_monorepo_package_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless typescript_monorepo_package_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
issue 2273: digest discriminator label reflects observed-advisory state for exposed preview findings; preview_limited safe action distinguishes complete-but-advisory repair packet

Command:
`cargo xtask goldens bless typescript_monorepo_package_local --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
