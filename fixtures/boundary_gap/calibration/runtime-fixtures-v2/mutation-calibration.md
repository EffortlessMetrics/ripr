# ripr mutation calibration report

Status: advisory

This report joins static seam evidence to supplied cargo-mutants runtime data. Runtime outcome vocabulary in this report comes from that runtime data; static ripr reports continue to use audit vocabulary only.

## Summary

| Metric | Count |
| --- | ---: |
| static_seams_total | 6 |
| mutants_total | 6 |
| matched_total | 4 |
| ambiguous_file_line_total | 1 |
| unmatched_mutants_total | 1 |
| static_without_runtime_total | 0 |

## Static/runtime agreement

| Agreement bucket | Count |
| --- | ---: |
| static_gap_and_runtime_signal | 2 |
| static_gap_without_runtime_signal | 1 |
| runtime_signal_without_static_gap | 1 |
| static_clean_and_runtime_clean | 1 |
| runtime_inconclusive | 1 |

Precision notes:

- runtime gap signals are imported runtime labels such as missed, survived, not_caught, or uncaught
- runtime clean signals are imported runtime labels such as caught or timeout
- static_gap_without_runtime_signal includes static gap seams with no matched runtime gap signal in this import
- ambiguous file/line runtime gap signals are counted as runtime_inconclusive until a seam_id or unambiguous location is available

### Runtime signals without static gaps

| Runtime mutant | Location | Runtime outcome | Static class | Confidence label | Reason |
| --- | --- | --- | --- | --- | --- |
| `m-v2-runtime-only-signal` | src/runtime_only.rs:99 | missed | `unmatched` | `runtime_only_signal` | runtime gap signal did not join to a static seam |

### Static gaps without runtime signals

| Seam | Class | Location | Confidence label | Reason |
| --- | --- | --- | --- | --- |
| `cal-v2-weak-snapshot-oracle` | `weakly_gripped` | src/summary.rs:36 | `contradicts_static_gap` | static gap seam matched runtime data without a runtime gap signal |

## Runtime Outcome Counts

| Runtime outcome | Count |
| --- | ---: |
| caught | 2 |
| missed | 4 |

## Matched Mutants

| Seam | Class | Oracle | Mutation operator | Runtime outcome | Join | Confidence label |
| --- | --- | --- | --- | --- | --- | --- |
| `cal-v2-mock-expectation` | `strongly_gripped` | `mock_expectation`/`strong` | delete gateway expectation call | caught | `seam_id` | `supports_static_clean` |
| `cal-v2-opaque-dispatch` | `reachable_unrevealed` | `opaque_dispatch`/`none` | replace dynamic dispatch target | missed | `seam_id` | `supports_static_gap` |
| `cal-v2-side-effect-observer` | `weakly_gripped` | `side_effect_observer`/`medium` | delete ledger side-effect call | missed | `seam_id` | `supports_static_gap` |
| `cal-v2-weak-snapshot-oracle` | `weakly_gripped` | `snapshot`/`weak` | replace serialized discount_reason field | caught | `seam_id` | `contradicts_static_gap` |

## Ambiguous File/Line Matches

| Runtime mutant | Location | Runtime outcome | Confidence label | Candidate seams |
| --- | --- | --- | --- | --- |
| `m-v2-ambiguous-opaque-dispatch` | src/routing.rs:58 | missed | `ambiguous_runtime_join` | `cal-v2-opaque-ambiguous-a`, `cal-v2-opaque-ambiguous-b` |

## Unmatched Runtime Mutants

| Location | Mutation operator | Runtime outcome | Test command |
| --- | --- | --- | --- |
| src/runtime_only.rs:99 | replace observer value | missed | cargo test runtime_only_observer |

## Static Seams Without Runtime Data

Every static seam matched at least one runtime mutant in the imported data.
