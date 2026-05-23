# First Successful PR Fixture Corpus

This manifest-owned corpus pins the `cargo xtask first-pr` start-here packet
for the first successful PR workflow.

The cases use explicit gap decision ledger inputs and expected
`start-here.{json,md}` outputs. They do not rerun analysis, edit source,
generate tests, call providers, run mutation testing, or change gate policy.

The boundary-gap case also carries a checked
[`ten-minute-demo.md`](boundary-gap/expected/ten-minute-demo.md) story. It
connects the first-pr packet to the reviewer-native outcome receipt so the demo
proves:

```text
before -> ripr first-pr -> top gap -> focused external proof -> ripr outcome -> receipt
```

The adopter-facing walkthrough is
[docs/demo/first-successful-pr.md](../../docs/demo/first-successful-pr.md).
