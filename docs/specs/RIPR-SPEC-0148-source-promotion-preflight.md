# RIPR-SPEC-0148: Source-promotion preflight receipt

Status: accepted

## Problem

Source promotion needs a deterministic, reviewable record of exact source and
swarm inputs before a join is constructed. This receipt is consumed from the
merged `ripr-swarm#3102` contract.

## Behavior

Current source consumers require preflight v2 and handoff acceptance v2. The
handoff includes lowercase-hex exact original manifest, complete bundle and
referenced packet bytes. Their raw digests and typed projections must agree
with fresh native #1609/#2766/#2769 decisions and the actual candidate tree.
Historical v1 geometry replay cannot supply current native acceptance. The
resolved-tree validator may run disposable v1 geometry diagnostics to retain
the J5 negative control, but an all-green diagnostic run must still pass the
final native v2 gate before it can receive a validated result. Current v2
inputs require native rereads before diagnostics and before that result. See
`docs/SOURCE_PROMOTION_PREFLIGHT.md` for bounds and transition behavior.
`cargo xtask source-promotion preflight` consumes complete source and swarm
parent SHAs plus explicit local repository roots and mandatory native selection/qualification inputs. It verifies origin identity,
exact commit identity, the held source main, and swarm-parent reachability. A
disposable repository fetches both exact objects, computes the merge base and
separately named all-reachable/first-parent counts, inventories changed paths,
and runs `git merge-tree --write-tree --name-only -z` for machine-readable
conflict-path evidence. This requires Git 2.38 or newer; older or malformed
Git versions fail closed before the merge probe.

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

### Consumed native acceptance (receipt v2)

The current schema is `ripr.source_promotion_preflight.v2`, paired with
native handoff acceptance v2. The earlier source
`82b2d7c262d229d5244263d458d10cd0189cb966` v1-only observation is historical;
current source consumers validate retained raw material and reread native
owner decisions before geometry and before final acceptance. Historical v1
geometry diagnostics cannot bypass the final current-v2 authority gate.

Before the geometry probe, the command consumes the independently recorded
#1609 selected-owner acceptance and #2769 complete-bundle acceptance using the
existing direct-manifest custody owner. It retrieves the native #1609, bound
#2766, and #2769 comments through a fixed-host, bounded, read-only GitHub
adapter. Comment ID/repository/issue and trusted author association are checked;
URLs and caller-written sidecars alone cannot admit a handoff.

The acceptance binds candidate SHA/tree/ref, complete raw manifest digest,
#2766 packet/decision-body digest, proof inputs, exact selected applicable-owner
roster, complete required-row denominator and full qualification-bundle digest.
Every selected required row is present, positive and nonzero; native-accepted
configured exclusions/deferred subjects are separately retained and cannot
silently become skipped selected subjects. No fixed seven-owner template list
is imposed. Missing/refused live inputs never fall back to historical custody.

The strict payload and count contracts are specified in
[SOURCE_PROMOTION_PREFLIGHT.md](../SOURCE_PROMOTION_PREFLIGHT.md#native-selection-and-complete-qualification-admission).
The command rechecks package/range/tree bytes through existing raw Git custody
and observes decisions and retained packets again before writing its receipt.
This consumes trusted operator judgments; it does not issue qualification,
cryptographically authenticate human approval, or provide atomic provenance.
Historical evidence and freeze-time `required_not_run` remain unchanged.

## Required Evidence

The receipt must preserve the exact identities, repository checks, range
denominators and digests (each commit id plus LF in listed order, including the
producer's existing empty-stream behavior), conflict and candidate inventories, reviewed tree,
version observations, and invalidation rules named above. The immutable swarm
ref must be the exact fully-qualified protected candidate tag
`refs/tags/ripr-release-<requested-version>-<SWARM_PARENT>` selected by the
release transaction's active tag ruleset, and it must resolve to the exact
`SWARM_PARENT` SHA. Legacy `refs/ripr/` refs, short tag names, branch refs, and
other movable or mismatched refs are invalid.
- native selection/qualification decisions and complete accepted packet identities agree;
- complete parent SHAs resolve exactly in their named repositories;
- required protected candidate tag uses
  `refs/tags/ripr-release-<version>-<SWARM_PARENT>` and resolves in the
  supplied swarm repository to exactly SWARM_PARENT; the local verifier ref
  `refs/ripr/release-<version>-<SWARM_PARENT>` is a separate release-control
  value and is not accepted as the preflight input;
- source parent equals the declared current source main;
- swarm parent is an ancestor of the declared swarm main;
- origin remotes identify the declared repositories;
- merge base, both denominator variants, and ordered digest recipe are present;
- disposable merge diagnostics and machine-readable conflict paths are present;
- automatic preview-tree output is distinct from an optional reviewed
  resolved-tree input;
- JSON and Markdown are deterministic projections with no temporary path or
  capture timestamp;
- exact-parent version observations include Cargo.lock ripr and npm lock root;
- invalidation rules name changes to the source parent, swarm parent, declared
  main, immutable ref resolution, identity, ancestry, digest, conflict, and
  tree.

## Non-Goals

No join construction, conflict adjudication, release metadata change,
publication authorization, or artifact qualification.

## Acceptance Examples

- A changed parent, ref resolution, range digest, or reviewed input requires a
  new receipt and fails byte-identity binding.
- A clean preflight records its automatic preview tree without treating it as
  the reviewed resolution.

## Test Mapping

- `xtask/src/reports/release/candidate_harness/live_head/handoff/tests.rs`
  injects read-only native source responses to discriminate valid applicable
  subsets/configured exclusions from missing or untrusted native decisions,
  wrong issue/host/comment, stale candidate/manifest/#2766/proof-input/roster,
  incomplete or failed/skipped/zero rows, generic successful CI, unknown fields
  and tampered bundle/packet bytes. No real release is dispatched by tests.
- `source_promotion::tests::public_preflight_requires_complete_native_handoff_inputs`
  runs the public command through mandatory input and native-reference refusal
  before geometry; local geometry fixtures remain geometry-only evidence.

- `xtask/src/reports/source_promotion.rs` unit tests cover SHA validation,
  digest order, strict remote identity (including suffix-trick rejection),
  authority-path classification, fixture shape, and disposable conflicting and
  clean repository pairs, exact-parent version reads, and reviewed resolved-tree
  verification for an unreachable `git write-tree` object. They also cover
  source-promotion fixture linkage, missing changelog unknown-state handling,
  location-independent identity serialization, exclusive disposable-directory
  creation, and rejection of a non-ancestor swarm main.
- `fixtures/source_promotion/diverged-conflict.json` pins the discriminating
  divergent/conflict expectation.

The source-side consumer proof includes
`source_promotion_acceptance::tests::complete_v2_replays_and_refetches_all_three_native_owners`,
`retained_raw_inputs_discriminate_complete_projection_tampering`, and
`fresh_native_response_cannot_substitute_body_issuer_location_or_author`.
The exact-J verifier contract remains RIPR-SPEC-0149.

## Implementation Mapping

- `xtask/src/reports/source_promotion.rs`
- `xtask/src/reports/release/candidate_harness/live_head/handoff.rs`
- `xtask/src/reports/release/candidate_harness/live_head/handoff/native.rs`
- `xtask/src/command.rs`
- `xtask/src/dispatch.rs`
- `docs/SOURCE_PROMOTION_PREFLIGHT.md`
- `xtask/src/reports/source_promotion_acceptance.rs`
- `xtask/src/reports/source_promotion_acceptance/native.rs`
- `xtask/src/reports/source_promotion_acceptance/material.rs`
- `xtask/src/reports/source_promotion_verify.rs`

## Metrics

Receipt generation and downstream verification retain ancestry counts and
ordered digests; unit-test pass rate is the local proof metric.
