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
