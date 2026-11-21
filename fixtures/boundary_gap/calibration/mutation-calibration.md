# ripr mutation calibration report

Status: advisory

This report joins static seam evidence to supplied cargo-mutants runtime data. Runtime outcome vocabulary in this report comes from that runtime data; static ripr reports continue to use audit vocabulary only.

## Summary

| Metric | Count |
| --- | ---: |
| static_seams_total | 1 |
| mutants_total | 1 |
| matched_total | 1 |
| ambiguous_file_line_total | 0 |
| unmatched_mutants_total | 0 |
| static_without_runtime_total | 0 |

## Static/runtime agreement

| Agreement bucket | Count |
| --- | ---: |
| static_gap_and_runtime_signal | 0 |
| static_gap_without_runtime_signal | 0 |
| runtime_signal_without_static_gap | 0 |
| static_clean_and_runtime_clean | 1 |
| runtime_inconclusive | 0 |

Precision notes:

- runtime gap signals are imported runtime labels such as missed, survived, not_caught, or uncaught
- runtime clean signals are imported runtime labels such as caught or timeout
- static_gap_without_runtime_signal includes static gap seams with no matched runtime gap signal in this import
- ambiguous file/line runtime gap signals are counted as runtime_inconclusive until a seam_id or unambiguous location is available

### Runtime signals without static gaps

No imported runtime gap signals lacked a matching static gap.

### Static gaps without runtime signals

No static gap seams lacked a runtime gap signal in this import.

## Runtime Outcome Counts

| Runtime outcome | Count |
| --- | ---: |
| caught | 1 |

## Matched Mutants

| Seam | Class | Oracle | Mutation operator | Runtime outcome | Join | Confidence label |
| --- | --- | --- | --- | --- | --- | --- |
| `67fc764ba37d77bd` | `strongly_gripped` | `exact_value`/`strong` | replace >= with > | caught | `seam_id` | `supports_static_clean` |

## Ambiguous File/Line Matches

No runtime mutants matched multiple static seams at the same file and line.

## Unmatched Runtime Mutants

All imported runtime mutants matched a static seam.

## Static Seams Without Runtime Data

Every static seam matched at least one runtime mutant in the imported data.
