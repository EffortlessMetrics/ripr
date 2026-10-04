# Executed-control packet

Schema: `executed_control_packet` version `1`
Source: `EffortlessMetrics/ripr-swarm`

This packet distinguishes executed discriminating controls from ordinary positive tests and review prose. It does not inspect live GitHub, enforce merge eligibility, or rewrite historical execution as pass.

## Obligations

- `claim:example:removed-guard` (required; removed_guard; expected `fails_before_passes_after`; subject `cargo test -p example -- removed_guard_control` on `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`; substitute none)

## Results

- `claim:example:removed-guard`: state=`passed`; evidence=`executed_discriminating_control`; observed=`failed_before_passed_after`; head=`aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`; command=`cargo test -p example -- removed_guard_control`; artifact `artifact:removed-guard-log` `sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`
- `claim:example:removed-guard`: state=`failed`; evidence=`executed_discriminating_control`; observed=`rejected_wrong_implementation`; head=`bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb`; command=`cargo test -p example -- removed_guard_control`; artifact `artifact:removed-guard-log` `sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`

## Satisfaction

`passed` requires an executed discriminating control that exercised the named wrong implementation. Ordinary positive tests, review prose, and structural-discrimination arguments cannot satisfy an obligation. `not_run`, `not_proven`, `substituted`, and `instrument_failure` stay explicit; only a substitute declared on the obligation can satisfy in place of a pass.

- `claim:example:removed-guard` (required) satisfies; recorded state=`passed`
