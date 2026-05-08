# Calibrated gate fixture matrix

These fixtures pin Campaign 15 gate decisions over existing PR-time evidence.
They are static policy projections: no source edits, generated tests, runtime
mutation execution, GitHub posting, or generated workflow default changes.

| Case | Purpose |
| --- | --- |
| `visible-only-advisory` | Default visibility records PR guidance without blocking. |
| `acknowledged-waiver` | `ripr-waive` produces a visible acknowledged decision. |
| `baseline-check-existing` | A baseline-known finding stays visible and non-blocking. |
| `calibrated-high-confidence-new-gap` | A new calibrated useful finding blocks only in explicit calibrated mode. |
| `summary-and-suppressed` | Summary-only and configured-off findings remain visible without blocking. |
| `missing-input` | Missing required PR guidance produces a config error and report. |
| `calibration-disagreement` | Noisy calibration keeps an otherwise eligible finding advisory. |
