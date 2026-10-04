# Executed-control packet

Schema: `executed_control_packet` version `1`
Source: `EffortlessMetrics/ripr-swarm`

This packet distinguishes executed discriminating controls from ordinary positive tests and review prose. It does not inspect live GitHub, enforce merge eligibility, or rewrite historical execution as pass.

## Obligations

- `issue:3858:eager-file-count-removal-control` (required; removed_guard; expected `fails_before_passes_after`; subject `cargo test -- removed_guard_or_eager_file_count_control` on `aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`; substitute none)

## Results

- `issue:3858:eager-file-count-removal-control`: state=`not_proven`; evidence=`review_prose`; observed=`not_executed`; head=`aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`; command=`cargo test -- removed_guard_or_eager_file_count_control`; no retained artifact; limitation: Issue #3858 / PR #4063 recorded no retained eager or removed-guard execution artifact. Execution is not_proven and must not be rewritten as passed.

## Satisfaction

`passed` requires an executed discriminating control that exercised the named wrong implementation. Ordinary positive tests, review prose, and structural-discrimination arguments cannot satisfy an obligation. `not_run`, `not_proven`, `substituted`, and `instrument_failure` stay explicit; only a substitute declared on the obligation can satisfy in place of a pass.

- `issue:3858:eager-file-count-removal-control` (required) does_not_satisfy; recorded state=`not_proven`
