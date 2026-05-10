# ripr mutation calibration report

Status: advisory

This report joins static seam evidence to supplied cargo-mutants runtime data. Runtime outcome vocabulary in this report comes from that runtime data; static ripr reports continue to use audit vocabulary only.

## Summary

| Metric | Count |
| --- | ---: |
| static_seams_total | 9 |
| mutants_total | 8 |
| matched_total | 6 |
| ambiguous_file_line_total | 1 |
| unmatched_mutants_total | 1 |
| static_without_runtime_total | 1 |

## Static/runtime agreement

| Agreement bucket | Count |
| --- | ---: |
| static_gap_and_runtime_signal | 1 |
| static_gap_without_runtime_signal | 3 |
| runtime_signal_without_static_gap | 2 |
| static_clean_and_runtime_clean | 1 |
| runtime_inconclusive | 2 |

Precision notes:

- runtime gap signals are imported runtime labels such as missed, survived, not_caught, or uncaught
- runtime clean signals are imported runtime labels such as caught or timeout
- static_gap_without_runtime_signal includes static gap seams with no matched runtime gap signal in this import
- ambiguous file/line runtime gap signals are counted as runtime_inconclusive until a seam_id or unambiguous location is available

### Runtime signals without static gaps

| Runtime mutant | Location | Runtime outcome | Static class | Confidence label | Reason |
| --- | --- | --- | --- | --- | --- |
| `m-static-clean-runtime-signal` | seam:cal-static-clean-runtime-signal | missed | `strongly_gripped` | `contradicts_static_clean` | runtime gap signal joined to a static-clean seam |
| `m-unmatched-runtime-signal` | src/pricing.rs:99 | missed | `unmatched` | `runtime_only_signal` | runtime gap signal did not join to a static seam |

### Static gaps without runtime signals

| Seam | Class | Location | Confidence label | Reason |
| --- | --- | --- | --- | --- |
| `cal-static-gap-runtime-clean` | `weakly_gripped` | src/pricing.rs:20 | `contradicts_static_gap` | static gap seam matched runtime data without a runtime gap signal |
| `cal-static-gap-no-runtime` | `ungripped` | src/pricing.rs:60 | `no_runtime_data` | static gap seam has no matched runtime record in this import |
| `cal-file-line-gap-clean` | `weakly_gripped` | src/pricing.rs:80 | `contradicts_static_gap` | static gap seam matched runtime data without a runtime gap signal |

## Runtime Outcome Counts

| Runtime outcome | Count |
| --- | ---: |
| caught | 3 |
| error | 1 |
| missed | 4 |

## Matched Mutants

| Seam | Class | Oracle | Mutation operator | Runtime outcome | Join | Confidence label |
| --- | --- | --- | --- | --- | --- | --- |
| `cal-file-line-gap-clean` | `weakly_gripped` | `exact_value`/`strong` | replace is_some with is_none | caught | `file_line` | `contradicts_static_gap` |
| `cal-static-clean-runtime-clean` | `strongly_gripped` | `exact_error_variant`/`strong` | replace error variant | caught | `seam_id` | `supports_static_clean` |
| `cal-static-clean-runtime-inconclusive` | `strongly_gripped` | `side_effect_observer`/`strong` | delete side-effect call | error | `seam_id` | `no_runtime_data` |
| `cal-static-clean-runtime-signal` | `strongly_gripped` | `exact_value`/`strong` | replace >= with > | missed | `seam_id` | `contradicts_static_clean` |
| `cal-static-gap-runtime-clean` | `weakly_gripped` | `exact_value`/`strong` | replace > with >= | caught | `seam_id` | `contradicts_static_gap` |
| `cal-static-gap-runtime-signal` | `weakly_gripped` | `exact_value`/`strong` | replace >= with > | missed | `seam_id` | `supports_static_gap` |

## Ambiguous File/Line Matches

| Runtime mutant | Location | Runtime outcome | Confidence label | Candidate seams |
| --- | --- | --- | --- | --- |
| `m-ambiguous-file-line` | src/pricing.rs:70 | missed | `ambiguous_runtime_join` | `cal-ambiguous-candidate-a`, `cal-ambiguous-candidate-b` |

## Unmatched Runtime Mutants

| Location | Mutation operator | Runtime outcome | Test command |
| --- | --- | --- | --- |
| src/pricing.rs:99 | replace arithmetic expression | missed | cargo test external_runtime_signal |

## Static Seams Without Runtime Data

Sample only; see JSON `static_without_runtime_total` for the full count.

| Seam | Kind | Class | Location | Confidence label |
| --- | --- | --- | --- | --- |
| `cal-static-gap-no-runtime` | `predicate_boundary` | `ungripped` | src/pricing.rs:60 | `no_runtime_data` |
