# Fixture Corpus: swarm-plan-packet-corpus

Spec: RIPR-SPEC-0057

## Given

`ripr-swarm plan` consumes existing `actionable-gaps.json` packets emitted from
canonical actionable evidence. Raw findings are present only as supporting
evidence.

## When

The plan ranks packet fixtures for swarm readiness.

## Then

Each case pins one packet shape and its expected planning outcome:

- high-confidence boundary assertion packet;
- exact error variant packet;
- static-only predicate-boundary packet requiring operator judgment;
- output observer packet;
- blocked static limitation packet;
- missing verify command packet;
- missing receipt command packet;
- missing must-not-change boundary packet;
- missing allowed-edit-surface packet.

## Must Not

Fixtures must not imply raw-finding consumption, source edits, provider calls,
generated tests, mutation execution, receipt creation, retry loops, public badge
changes, PR/CI rendering changes, autonomous merges, or production-code edits by
default.
