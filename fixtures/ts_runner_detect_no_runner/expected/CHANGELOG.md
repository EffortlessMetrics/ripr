# Golden Output Changes

## Pending

Reason:
Initial golden for fail-closed typescript_test_runner_unresolved: package.json present but no known runner in devDeps or scripts.test; typescript_test_runner NOT emitted. TS must-use roadmap item 3 (RIPR-SPEC-0085).

Command:
`cargo xtask goldens bless ts_runner_detect_no_runner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless ts_runner_detect_no_runner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless ts_runner_detect_no_runner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
issue 2273: digest discriminator label reflects observed-advisory state for exposed preview findings; preview_limited safe action distinguishes complete-but-advisory repair packet

Command:
`cargo xtask goldens bless ts_runner_detect_no_runner --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
