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

## Pending

Reason:
AVA t.is exact-value oracle now recognized (RIPR-SPEC-0085 execution-context t.* shapes): weakly_exposed -> exposed, oracle exact_value/strong; stays preview_advisory_only

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Render receiver-gated AVA evidence as t.is(...) and label already-observed actionability as TypeScript t.* evidence while keeping preview advisory.

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
bound default human output to start-here triage; human-full preserves exhaustive evidence

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2103: additive changed_files_by_language field and changed_rust_files now Rust-only count

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
issue 2273: digest discriminator label reflects observed-advisory state for exposed preview findings; preview_limited safe action distinguishes complete-but-advisory repair packet

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
#2567: default human render no longer prints a 'Hidden: 0 lower-priority finding(s) omitted' block when nothing was omitted; the format pointers now sit under a 'More:' heading. Formatting-only drift; no evidence, class, or JSON change.

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
Issue #2598: default human output now exposes bounded explain and context follow-up commands for the selected finding.

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`

## Pending

Reason:
show preview language metadata in human finding digests

Command:
`cargo xtask goldens bless ts_runner_detect_ava_script --reason "..."`

Updated:
- `expected/check.json`
- `expected/human.txt`
