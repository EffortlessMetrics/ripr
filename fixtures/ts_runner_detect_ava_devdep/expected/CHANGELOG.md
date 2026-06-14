# Golden Output Changes

## Pending

Reason:
Initial golden for ava devdep detection: typescript_test_runner: ava emitted from devDependencies; typescript_verify_command: ava tests/math.test.ts inferred. TS must-use roadmap item 3 (RIPR-SPEC-0085).

Command:
`cargo xtask goldens bless ts_runner_detect_ava_devdep --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
text-only update: weak_oracle_missing_summary _-arm now mentions toThrow exact-payload forms (RIPR-SPEC-0097)

Command:
`cargo xtask goldens bless ts_runner_detect_ava_devdep --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
