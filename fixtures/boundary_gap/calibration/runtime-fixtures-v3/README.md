# Runtime Fixtures V3 Calibration Corpus

This directory contains checked imported-runtime samples for the Lane 1
static/runtime confidence expansion. The inputs are synthetic and repo-local:
RIPR imports them through `cargo xtask mutation-calibration`; it does not run
mutation testing as part of fixture validation.

## Cases

| Case | Evidence class | Expected label | Claim | Must not claim |
| --- | --- | --- | --- | --- |
| `cal-v3-custom-helper-outcome` | custom assertion helper outcomes | `supports_static_gap` | Runtime data supports the existing static gap for the checked helper outcome sample. | Custom helpers are not globally calibrated. |
| `cal-v3-table-boundary-outcome` | table-driven boundary outcomes | `supports_static_clean` | Runtime data supports the static-clean table-boundary sample. | Runtime data must not create or remove a static seam. |
| `cal-v3-builder-override-outcome` | builder override outcomes | `contradicts_static_gap` | Runtime data lowers confidence in this checked static gap sample. | Builder override cases are not globally calibrated. |
| `cal-v3-cross-file-constant-boundary` | cross-file constant boundary outcomes | `no_runtime_data` | The static gap remains visible without runtime calibration data. | Absence of runtime data must not hide the static gap. |
| `cal-v3-snapshot-field-discriminator` | snapshot field-discriminator outcomes | `contradicts_static_clean` | Runtime data raises a calibration concern for the static-clean snapshot-field sample. | The static grip class must not change from runtime data alone. |
| `cal-v3-mock-expectation-mismatch` | mock expectation mismatch outcomes | `supports_static_gap` | Runtime data supports the existing static gap for a checked mock argument mismatch. | Mock expectations are not globally calibrated. |
| `m-v3-ambiguous-builder` | ambiguous file/line join | `ambiguous_runtime_join` | Multiple candidate seams remain ambiguous. | The report must not pick one candidate seam. |
| `m-v3-runtime-only-signal` | unmatched runtime signal | `runtime_only_signal` | Runtime-only signal remains calibration context. | Runtime-only signal must not create a static gap. |

## Update Command

```bash
cargo xtask mutation-calibration . --mutants-json fixtures/boundary_gap/calibration/runtime-fixtures-v3/runtime-mutants.json --repo-exposure-json fixtures/boundary_gap/calibration/runtime-fixtures-v3/repo-exposure.json
```
