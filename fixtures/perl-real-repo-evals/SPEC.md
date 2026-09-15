# Perl Real-Repo Eval Corpus

Spec: RIPR-SPEC-0064

Tracker: `.ripr/goals/perl-repair-routing.toml`

## Given

Perl repair-routing has a fixture-scoped producer-to-consumer proof in
`crates/ripr/tests/perl_two_binary_harness.rs`. That harness runs a real
`perl-ripr-facts` compatible exporter against the CPAN-style
`fixtures/perl_cpan_alpha/input` project when the exporter is available.

## When

An eval case records a Perl producer launchpoint, the CPAN-style input project,
the diff variant, the expected static outcome, and the claim boundary for the
producer-dependent run. These cases are checked corpus records, not support-tier
promotion receipts.

## Then

The corpus must keep the three honest Perl outcomes visible:

- actionable weak-oracle evidence for the boundary change;
- already-observed exact-oracle evidence with no repair packet;
- a named `dynamic_dispatch` limitation with no repair packet.

The corpus also records that public repair packets, agent packets, default
gate/badge/RIPR Zero authority, and support-tier promotion remain out of scope
until the real-repo metrics and shared renderer work land.

## Must Not

- Do not treat the local CPAN fixture as the required five real Perl repos.
- Do not claim top-1 precision, verify-command validity, false-actionable rate,
  or before/after receipt thresholds from this corpus.
- Do not claim public repair-packet authority or support-tier promotion.
- Do not turn committed regression packets into producer proof.
