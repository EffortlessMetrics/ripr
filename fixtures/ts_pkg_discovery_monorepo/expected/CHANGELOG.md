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
