# Golden Output Changes

## Pending

Reason:
Initial golden for single-package TypeScript discovery: package_root, workspace_root, framework_hint=jest, runner_hint=npm evidence lines added by RIPR-SPEC-0085 PR 2

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
PR 3: add typescript_verify_command evidence line; jest tests/math.test.ts inferred from jest framework + npm runner

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
PR5: additive oracle metadata lines (observed/expected/confidence/evidence_ref)

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0088 §PR8: named limitation now surfaced for blocked TS packet

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0094: additive relation_reason+relation_confidence fields now populated in TS related_tests (DirectOwnerCall->direct_owner_call/high, ImportedOwnerCall->import_path_affinity/medium, etc.); existing fixture classes, oracle strength, and probe data unchanged

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive typescript_test_runner evidence field (TS must-use roadmap item 3)

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
issue 2273: digest discriminator label reflects observed-advisory state for exposed preview findings; preview_limited safe action distinguishes complete-but-advisory repair packet

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
show preview language metadata in human finding digests

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2659: finding navigation commands now preserve the analyzed root, diff or artifact scope and shell-safe identity.

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
