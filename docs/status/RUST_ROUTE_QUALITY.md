# Rust route-quality status

Status: incomplete
Campaign: `rust-one-shot-evidence-to-repair`
Tracker: [RIPR-PLAN-0062](../../plans/rust-one-shot-evidence-to-repair.md)
Corpus issue: [#1560](https://github.com/EffortlessMetrics/ripr-swarm/issues/1560)
CallPresence issue: [#1543](https://github.com/EffortlessMetrics/ripr-swarm/issues/1543)

## Current state

The targeted-rerun contract is shipped infrastructure. The governed Rust
route-quality corpus is authorized for three internal adopting repositories,
but it contains zero counted attempts. Run
`cargo xtask rust-repair-trust-report` for the current denominator-preserving
scorecard; the expected state before the pilot is `limited`.

CallPresence remains separately limited. Positive analyzer unit tests are
synthetic evidence, and the latest bounded repository scan is negative
evidence on an older SHA. Neither establishes a real producer-owned route.
Corpus collection does not depend on finding a positive CallPresence example;
the corpus measures that limitation when it occurs. Final campaign closeout
requires the corpus threshold and either a qualifying CallPresence receipt or
an explicit durable limitation disposition.

## Scorecard boundary

The report measures route evidence, not developer or agent performance. It
reports authorized repositories, supplied and eligible attempts, excluded
rows and reasons, movement counts, one-attempt improvement, attempts to first
improvement, repair rounds, false actionability, known-impossible
recommendations, missing route fields, general and CallPresence limitation
frequency, parity failures, and artifact archaeology. Rates remain null when
their denominator is zero, and every rate carries explicit numerator and
denominator fields.

The corpus does not establish runtime mutation results, coverage, universal
Rust-seam support, or a support-tier claim beyond the observed repositories,
revisions, configuration, receipts, and remaining limitations.
