# Golden Output Changes

## Pending

Reason:
Initial golden for ava detection from composite scripts.test (Ky pattern: "xo && npm run build && ava"); typescript_test_runner: ava emitted via script-name fallback. TS must-use roadmap item 3 (RIPR-SPEC-0085).

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
