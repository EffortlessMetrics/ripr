# RIPR-SPEC-0148: Source-promotion preflight receipt

Status: accepted

## Problem

Source promotion needs a deterministic, reviewable record of exact source and
swarm inputs before a join is constructed. This receipt is consumed from the
merged `ripr-swarm#3102` contract.

## Behavior

The `ripr.source_promotion_preflight.v1` receipt binds exact source and swarm
parents, the merge base, immutable swarm-ref resolution, repository identity,
separately named all-reachable and first-parent ancestry counts and ordered
SHA-256 digests, dry-merge conflict inventory, reviewed resolved-tree identity,
version observations, and deterministic invalidation rules. Its automatic
`preview_tree` is never a final resolution.

Preflight is evidence only: it does not create the join, adjudicate conflicts,
change release metadata, or authorize publication. Canonical receipt bytes are
the exact UTF-8 bytes written by the producer, including pretty-print
whitespace and the trailing LF; the verifier parses those bytes only after
hashing them. `preflight_sha256` covers exactly the file bytes, not a
reserialized JSON value. Any changed input requires a new receipt and reviewed
resolution manifest whose binding matches those bytes.

## Required Evidence

The receipt must preserve the exact identities, repository checks, range
denominators and digests (each commit id plus LF in listed order, including the
producer's existing empty-stream behavior), conflict and candidate inventories, reviewed tree,
version observations, and invalidation rules named above. Immutable swarm refs
must use the producer-controlled `refs/ripr/` namespace and an exact SHA pin.

## Non-Goals

No join construction, conflict adjudication, release metadata change,
publication authorization, or artifact qualification.

## Acceptance Examples

- A changed parent, ref resolution, range digest, or reviewed input requires a
  new receipt and fails byte-identity binding.
- A clean preflight records its automatic preview tree without treating it as
  the reviewed resolution.

## Test Mapping

The source-side preflight command and its synthetic repository tests produce
the receipt consumed by the verifier in RIPR-SPEC-0149.

## Implementation Mapping

The producer is `ripr-swarm` source-promotion preflight tooling; the source
consumer is `docs/SOURCE_PROMOTION.md` and the exact-J verifier module.

## Metrics

Receipt generation and downstream verification retain ancestry counts and
ordered digests; unit-test pass rate is the local proof metric.
