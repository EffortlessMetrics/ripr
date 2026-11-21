# Recommendation Calibration

Status: advisory

## Summary

| Metric | Count |
| --- | ---: |
| recommendations_evaluated | 10 |
| useful | 2 |
| noisy | 1 |
| false_annotations | 4 |
| summary_only_correct | 2 |
| suppressed_correctly | 2 |
| target_file_correct | 6 |
| static_improved | 2 |
| static_unchanged | 3 |
| static_regressed | 0 |
| unknown | 0 |

## Top Recommendation

- `8f7fa8644fd12280` `correct` `useful`
- placement: `src/pricing.rs`:88 `exact_seam_line`
- why: The recommendation points at the changed seam line and names the expected boundary discriminator and test target.
- suggested test: `tests/pricing.rs` `correct`
- static movement: `improved` from `outcome_receipt`

## Recommendations

| Rank | Seam | Source | Placement | Outcome | Target | Movement |
| ---: | --- | --- | --- | --- | --- | --- |
| 1 | `8f7fa8644fd12280` | `useful_exact_line_boundary` | `correct` | `useful` | `correct` | `improved` |
| 2 | `8f7fa8644fd12280` | `noisy_owner_fallback` | `correct` | `noisy` | `correct` | `unchanged` |
| 3 | `8f7fa8644fd12280` | `wrong_line_same_file_fallback` | `wrong_line` | `wrong_line` | `correct` | `unchanged` |
| 4 | `calibration-already-covered` | `already_covered_visible` | `correct` | `already_covered` | `not_applicable` | `unchanged` |
| 5 | `8f7fa8644fd12280` | `summary_only_expected_boundary` | `summary_only_expected` | `summary_only_correct` | `correct` | `unknown` |
| 6 | `calibration-macro-heavy` | `macro_heavy_summary_only` | `summary_only_expected` | `summary_only_correct` | `correct` | `unknown` |
| 7 | `calibration-trait-generic` | `trait_generic_wrong_target` | `correct` | `wrong_target` | `wrong_target` | `unknown` |
| 8 | `calibration-async-error` | `async_error_boundary_useful` | `correct` | `useful` | `correct` | `improved` |

## Suppressed

| Seam | Reason | Quality | Outcome |
| --- | --- | --- | --- |
| `8f7fa8644fd12280` | `severity_off` | `suppressed_correctly` | `suppressed_correctly` |
| `calibration-generated-migration` | `generated_or_migration` | `suppressed_correctly` | `suppressed_correctly` |

## Limits

Advisory recommendation-quality evidence only; no telemetry, generated tests, source edits, runtime execution, or CI blocking.
