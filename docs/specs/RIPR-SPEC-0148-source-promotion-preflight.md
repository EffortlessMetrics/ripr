# RIPR-SPEC-0148: Source-promotion preflight receipt

Status: accepted (consumed from the merged `ripr-swarm#3102` contract)

The `ripr.source_promotion_preflight.v1` receipt binds exact source and swarm
parents, the merge base, immutable swarm-ref resolution, repository identity,
separately named all-reachable and first-parent ancestry counts and ordered
SHA-256 digests, dry-merge conflict inventory, reviewed resolved-tree identity,
version observations, and deterministic invalidation rules. Its automatic
`preview_tree` is never a final resolution.

Preflight is evidence only: it does not create the join, adjudicate conflicts,
change release metadata, or authorize publication. Any changed input requires
a new byte-identical receipt and a new reviewed resolution manifest.
