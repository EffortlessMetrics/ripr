# Golden Output Changes

## Pending

Reason:
Initial golden for monorepo TypeScript discovery: package_root=packages/auth, workspace_root=. (pnpm-workspace.yaml), framework_hint=jest, runner_hint=pnpm evidence lines added by RIPR-SPEC-0085 PR 2

Command:
`cargo xtask goldens bless ts_pkg_discovery_monorepo --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
PR 3: add typescript_verify_command evidence line; jest tests/token.test.ts inferred from jest framework (path relative to packages/auth package_root)

Command:
`cargo xtask goldens bless ts_pkg_discovery_monorepo --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
PR5: additive oracle metadata lines

Command:
`cargo xtask goldens bless ts_pkg_discovery_monorepo --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0088 §PR8: named limitation now surfaced for blocked TS packet

Command:
`cargo xtask goldens bless ts_pkg_discovery_monorepo --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_pkg_discovery_monorepo --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_pkg_discovery_monorepo --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive: add related_tests_total cap field (mirrors repo-exposure pattern)

Command:
`cargo xtask goldens bless ts_pkg_discovery_monorepo --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0094: additive relation_reason+relation_confidence fields now populated in TS related_tests (DirectOwnerCall->direct_owner_call/high, ImportedOwnerCall->import_path_affinity/medium, etc.); existing fixture classes, oracle strength, and probe data unchanged

Command:
`cargo xtask goldens bless ts_pkg_discovery_monorepo --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
additive typescript_test_runner evidence field (TS must-use roadmap item 3)

Command:
`cargo xtask goldens bless ts_pkg_discovery_monorepo --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
