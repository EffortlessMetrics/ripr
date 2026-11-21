# Recommendation Calibration Outcome Receipts

These files pin the optional review guidance outcome receipt shape for
`RIPR-SPEC-0013`.

They are static local artifacts. They do not send telemetry, call external
services, post comments, edit source, generate tests, run mutation testing, or
change CI blocking behavior.

Files:

- `useful.json` records an actionable exact-line recommendation.
- `noisy.json` records a visible recommendation that is expected to be noisy in
  this review context.
- `wrong-line.json` records a line-level recommendation on the wrong changed
  line.
- `already-covered.json` records a recommendation that should be treated as
  already covered by a nearby focused test change.
- `wrong-target.json` records a recommendation with the wrong suggested test
  target.
- `summary-only-correct.json` records a recommendation correctly kept out of
  inline placement.
- `suppressed-correctly.json` records a recommendation hidden by configured
  severity.

The recommendation calibration report treats missing receipts as
`unknown`; absence of feedback must not be interpreted as useful or noisy.
