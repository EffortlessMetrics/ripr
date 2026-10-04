# Rust judged-panel calibration scorecard

Authority: `#4795`. Structural judgments stay independent of runtime results.

## Counts

- selected: 21
- judged: 21
- calibration_eligible: 0
- attempted: 0
- completed: 0
- inconclusive: 0

## Identities

- selection: `sha256:69c9b45616bd4c9bd13e8da0d668e52c39bca32485f93335589009e77c3fdd72`
- judgments: `sha256:04ad4167e930af01d48041e518796f1167d9c3347b5e51445976c80586f6c869`
- rolling observation: `sha256:0441b6329f7cfcbfa242213003cdf4d8f5cc9545d6168ebc02405998952ea940`
- digest: `sha256:4d69c040435ff1ac8e6b8824271a55144dfbbe5298cfae9353b16670564fbe78`

## Static × runtime matrix

| static | runtime | count | cases |
| --- | --- | --- | --- |
| `should_gap` | `not_run` | 8 | p1617-command-rows-quiet, p1617-dispatch-rows-gap, p1705-passthrough-rows-quiet, p1706-wiring-rows-gap, p1741-helper-rows-quiet, p1741-pair-rows-gap, p1745-catalog-row-quiet, s3755-weak-rows-gap |
| `should_limit` | `not_run` | 3 | s2864-fast-mode-notice-limit, s3866-doctor-packet-subprocess-limit, s3956-pilot-baseline-file-limit |
| `should_stay_quiet` | `not_run` | 10 | p1705-headline-rows-gap, p1705-headline-survivor-quiet, p1706-helper-rows-quiet, p1731-quiet-test-only, p1732-quiet-test-only, p1744-quiet-test-only, p1745-triple-rows-gap, p1745-wedge-rows-quiet, s3755-aligned-rows-quiet, s3759-match-arm-quiet |

## Candidates

- false_actionable: not_measurable (numerator 0, denominator 0)
- false_exposed: not_measurable (numerator 0, denominator 0)
- static_under_credit: not_measurable (numerator 0, denominator 0)
- limitation_correct: 0/3 (numerator 0, denominator 3)
- wrong_target: 0/21 (numerator 0, denominator 21)

Survived mutants are retained without an automatic false-exposed conclusion.

## Cases

- `p1617-command-rows-quiet`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_gap`
- `p1617-dispatch-rows-gap`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_gap`
- `p1705-headline-rows-gap`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_stay_quiet`
- `p1705-headline-survivor-quiet`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_stay_quiet`
- `p1705-passthrough-rows-quiet`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_gap`
- `p1706-helper-rows-quiet`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_stay_quiet`
- `p1706-wiring-rows-gap`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_gap`
- `p1731-quiet-test-only`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_stay_quiet`
- `p1732-quiet-test-only`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_stay_quiet`
- `p1741-helper-rows-quiet`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_gap`
- `p1741-pair-rows-gap`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_gap`
- `p1744-quiet-test-only`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_stay_quiet`
- `p1745-catalog-row-quiet`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_gap`
- `p1745-triple-rows-gap`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_stay_quiet`
- `p1745-wedge-rows-quiet`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_stay_quiet`
- `s2864-fast-mode-notice-limit`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_limit`
- `s3755-aligned-rows-quiet`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_stay_quiet`
- `s3755-weak-rows-gap`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_gap`
- `s3759-match-arm-quiet`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_stay_quiet`
- `s3866-doctor-packet-subprocess-limit`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_limit`
- `s3956-pilot-baseline-file-limit`: eligibility `ineligible_unauthorized`, runtime `not_run`, terminal `confirmed_should_limit`

## Limits

- Runtime results cannot rewrite independently accepted structural judgments.
- A survived mutant is a review candidate, not an automatic false-exposed conclusion.
- A caught mutant on a static gap is an under-credit candidate, not an automatic analyzer defect.
- should_limit remains a static-boundary judgment even when a runtime experiment completes.
- Equivalent, failed, timed-out, unavailable, stale and inconclusive rows stay in the denominator.
- No denominator is not_measurable, never a fake zero percent.
- #3076 route-yield denominators are referenced and never merged.
- #4578 rolling observation identity is bound and never merged into this classification denominator.
- No single quality score, support-tier, release, or publication claim.
