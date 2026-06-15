# Golden Output Changes

## Pending

Reason:
Initial golden for fail-closed typescript_test_runner_unresolved: package.json present but no known runner in devDeps or scripts.test; typescript_test_runner NOT emitted. TS must-use roadmap item 3 (RIPR-SPEC-0085).

Command:
`cargo xtask goldens bless ts_runner_detect_no_runner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
