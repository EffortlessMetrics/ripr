# ripr mutation calibration report

Status: advisory

This report joins static seam evidence to supplied cargo-mutants runtime data. Runtime outcome vocabulary in this report comes from that runtime data; static ripr reports continue to use audit vocabulary only.

## Summary

| Metric | Count |
| --- | ---: |
| static_seams_total | 8 |
| mutants_total | 7 |
| matched_total | 5 |
| ambiguous_file_line_total | 1 |
| unmatched_mutants_total | 1 |
| static_without_runtime_total | 1 |

## Static/runtime agreement

| Agreement bucket | Count |
| --- | ---: |
| static_gap_and_runtime_signal | 2 |
| static_gap_without_runtime_signal | 2 |
| runtime_signal_without_static_gap | 2 |
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
| `m-v3-snapshot-field-discriminator` | seam:cal-v3-snapshot-field-discriminator | missed | `strongly_gripped` | `contradicts_static_clean` | runtime gap signal joined to a static-clean seam |
| `m-v3-runtime-only-signal` | src/runtime_only_v3.rs:99 | missed | `unmatched` | `runtime_only_signal` | runtime gap signal did not join to a static seam |

### Static gaps without runtime signals

| Seam | Class | Location | Confidence label | Reason |
| --- | --- | --- | --- | --- |
| `cal-v3-builder-override-outcome` | `weakly_gripped` | src/builder.rs:33 | `contradicts_static_gap` | static gap seam matched runtime data without a runtime gap signal |
| `cal-v3-cross-file-constant-boundary` | `weakly_gripped` | src/cross_file.rs:41 | `no_runtime_data` | static gap seam has no matched runtime record in this import |

## Runtime Outcome Counts

| Runtime outcome | Count |
| --- | ---: |
| caught | 2 |
| missed | 5 |

## Matched Mutants

| Seam | Class | Oracle | Mutation operator | Runtime outcome | Join | Confidence label |
| --- | --- | --- | --- | --- | --- | --- |
| `cal-v3-builder-override-outcome` | `weakly_gripped` | `exact_value`/`weak` | replace builder override threshold | caught | `seam_id` | `contradicts_static_gap` |
| `cal-v3-custom-helper-outcome` | `weakly_gripped` | `custom_assertion_helper`/`medium` | replace helper-covered threshold comparison | missed | `seam_id` | `supports_static_gap` |
| `cal-v3-mock-expectation-mismatch` | `weakly_gripped` | `mock_expectation`/`medium` | replace mock expectation amount argument | missed | `seam_id` | `supports_static_gap` |
| `cal-v3-snapshot-field-discriminator` | `strongly_gripped` | `snapshot_field`/`strong` | replace snapshot discount_code field | missed | `seam_id` | `contradicts_static_clean` |
| `cal-v3-table-boundary-outcome` | `strongly_gripped` | `exact_value`/`strong` | replace table threshold comparison | caught | `seam_id` | `supports_static_clean` |

## Ambiguous File/Line Matches

| Runtime mutant | Location | Runtime outcome | Confidence label | Candidate seams |
| --- | --- | --- | --- | --- |
| `m-v3-ambiguous-builder` | src/ambiguous_builder.rs:72 | missed | `ambiguous_runtime_join` | `cal-v3-ambiguous-builder-a`, `cal-v3-ambiguous-builder-b` |

## Unmatched Runtime Mutants

| Location | Mutation operator | Runtime outcome | Test command |
| --- | --- | --- | --- |
| src/runtime_only_v3.rs:99 | replace runtime-only observed value | missed | cargo test runtime_only_v3_observer |

## Static Seams Without Runtime Data

Sample only; see JSON `static_without_runtime_total` for the full count.

| Seam | Kind | Class | Location | Confidence label |
| --- | --- | --- | --- | --- |
| `cal-v3-cross-file-constant-boundary` | `predicate_boundary` | `weakly_gripped` | src/cross_file.rs:41 | `no_runtime_data` |
