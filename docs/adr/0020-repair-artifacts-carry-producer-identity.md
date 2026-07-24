# ADR 0020: Repair Artifacts Carry Producer Identity

Status: accepted

Date: 2026-07-22

Artifact ID: RIPR-ADR-0020

## Context

`ripr agent verify` compares saved static snapshots before producing movement
evidence. A path and plausible JSON shape do not establish that either input
was produced by RIPR, belongs to the selected repository, or is unchanged
since production. Treating those fields as sufficient lets fabricated input
reach movement and receipt consumers.

## Decision

Repo-exposure snapshots consumed by `agent verify` carry a producer-owned
`artifact` envelope containing:

- artifact kind and identity schema;
- producer tool/version and commitment canonicalization;
- analyzed root and Git HEAD when available;
- analysis format, mode, base revision, and worktree state;
- a SHA-256 commitment over the exact JSON bytes with the commitment value
  replaced by the fixed `raw_json_placeholder_v1` value.

The producer computes the commitment in a bounded-memory two-pass writer.
`agent verify` parses and validates this envelope, checks root and revision
compatibility, verifies the commitment, and reports `current`,
`dirty_worktree`, or `historical_noncurrent` currentness. Invalid or
unavailable identities fail before static movement is calculated.

This is an integrity/currentness boundary, not a signature system and not
runtime mutation proof. Signature, command execution, and receipt issuance
remain separate follow-up contracts under #1941.

## Consequences

Positive:

- Legacy or hand-authored snapshots cannot reach movement merely because they
  contain `seams` or `findings` fields.
- Large repo-exposure artifacts retain streaming output and bounded memory.
- Dirty and historical evidence stays inspectable with an explicit disclosure
  rather than being presented as current.

Costs and limits:

- Existing static calibration fixtures must be wrapped or generated through
  RIPR before `agent verify` can consume them.
- A producer marker and digest do not replace signing; a compromised producer
  can still emit a self-consistent artifact.
- `receipt` provenance and verification-command execution are intentionally not
  claimed by this decision.
