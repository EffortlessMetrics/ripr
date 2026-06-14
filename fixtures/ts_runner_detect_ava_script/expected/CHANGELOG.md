# Golden Output Changes

## Pending

Reason:
gap-3 honesty-clarity reword (RIPR-SPEC-0101): ava framework known via script-name detection, no lockfile → limitation reworden from typescript_runner_hint_unresolved to typescript_package_manager_unresolved; verify command unchanged.

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "gap-3 reword: framework-known case now emits typescript_package_manager_unresolved instead of typescript_runner_hint_unresolved (RIPR-SPEC-0101)"`

Updated:
- `expected/check.json`

## Pending

Reason:
Initial golden for ava detection from composite scripts.test (Ky pattern: "xo && npm run build && ava"); typescript_test_runner: ava emitted via script-name fallback. TS must-use roadmap item 3 (RIPR-SPEC-0085).

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
text-only update: weak_oracle_missing_summary _-arm now mentions toThrow exact-payload forms (RIPR-SPEC-0097)

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
RIPR-SPEC-0104: family-matched oracle selection excludes unknown-kind oracles from strongest computation; observe-summary rank 1->0 for heuristic relations, classification unchanged

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
delta-5: drop verify_command from missing list when runner resolved

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
