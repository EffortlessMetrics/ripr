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
