# First Successful PR Fixture Corpus

This manifest-owned corpus pins the `cargo xtask first-pr` start-here packet
for the first successful PR workflow.

The cases use explicit gap decision ledger inputs and expected
`start-here.{json,md}` outputs. They do not rerun analysis, edit source,
generate tests, call providers, run mutation testing, or change gate policy.

The adopter-facing walkthrough is
[docs/demo/first-successful-pr.md](../../docs/demo/first-successful-pr.md).

The canonical boundary-gap case also carries a case-local story at
[boundary-gap/README.md](boundary-gap/README.md). It connects the checked
`start-here` packet to the existing before/after targeted-test outcome receipt
so the fixture proves the full first-useful loop without source edits,
generated tests, provider calls, mutation execution, or gate changes.

The `python-preview-gap` case pins the preview-language bridge: an explicit
Python GapRecord from the gap decision ledger can become a `preview_limited`
`start-here` packet for a Python project root without requiring Cargo or
promoting Python beyond advisory preview status. Its outcome receipt fixtures
also show the same canonical Python gap closing, staying unchanged, and opening
from check-output before/after snapshots.

The `python-return-gap`, `python-exception-gap`, `python-field-gap`, and
`python-output-gap` cases
extend that receipt proof beyond predicate boundaries: broad return,
exception, field/object, and output/log evidence can become exact evidence and
close the same canonical Python gap while preserving preview/advisory language.
