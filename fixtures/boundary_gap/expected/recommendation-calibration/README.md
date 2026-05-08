# Recommendation Calibration Fixture Expectations

These files pin the Campaign 14 recommendation-calibration corpus for
`RIPR-SPEC-0013`.

They are static fixture artifacts. They do not post comments, edit source,
generate tests, run mutation testing, or change CI blocking behavior.

Files:

- `expectations.json` records expected calibration outcomes for PR-shaped
  guidance cases.
- `synthetic-pr-guidance.json` supplies compact PR-guidance-shaped inputs for
  cases that are not emitted by the existing boundary-gap PR guidance fixtures.
- `outcome-receipts/` pins optional local receipt examples for useful, noisy,
  wrong-line, already-covered, wrong-target, summary-only-correct, and
  suppressed-correctly feedback.

The corpus intentionally mixes existing checked PR guidance outputs with small
synthetic guidance records so the future report producer can prove:

- useful top recommendations;
- noisy and wrong-line recommendations;
- already-covered seams;
- correct summary-only fallback;
- suppression, configured-off, and generated/migration exclusions;
- macro-heavy, trait/generic, and async/error boundary cases.

Outcome receipts are inputs to the future report, not a posting or telemetry
surface. Missing receipts must remain `unknown` unless another local artifact
supplies bounded calibration evidence.
