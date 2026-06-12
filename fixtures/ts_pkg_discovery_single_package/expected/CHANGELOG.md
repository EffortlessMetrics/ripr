# Golden Output Changes

## Pending

Reason:
Initial golden for single-package TypeScript discovery: package_root, workspace_root, framework_hint=jest, runner_hint=npm evidence lines added by RIPR-SPEC-0085 PR 2

Command:
`cargo xtask goldens bless ts_pkg_discovery_single_package --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
