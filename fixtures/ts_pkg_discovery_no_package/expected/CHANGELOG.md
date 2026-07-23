# Golden Output Changes

## Pending

Reason:
Initial golden for no-package TypeScript discovery: typescript_package_root_unresolved limitation emitted (fail-closed), no package_root guessed, as required by RIPR-SPEC-0085 PR 2

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
PR 3: fail-closed typescript_test_runner_unresolved when no package.json found (package_root=None → no verify command)

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
PR5: additive oracle metadata lines (observed/expected/confidence/evidence_ref)

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0088 §PR8: named limitation now surfaced for blocked TS packet

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0094: additive relation_reason+relation_confidence fields now populated in TS related_tests (DirectOwnerCall->direct_owner_call/high, ImportedOwnerCall->import_path_affinity/medium, etc.); existing fixture classes, oracle strength, and probe data unchanged

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
issue 2273: digest discriminator label reflects observed-advisory state for exposed preview findings; preview_limited safe action distinguishes complete-but-advisory repair packet

Command:
`cargo xtask goldens bless ts_pkg_discovery_no_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
