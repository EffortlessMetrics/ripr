# RIPR-SPEC-0134: Repair Artifact Provenance

Status: accepted

Owner: product / swarm

Created: 2026-07-22

Linked ADRs:

- [ADR 0020](../adr/0020-repair-artifacts-carry-producer-identity.md)

Linked issues:

- [#1977](https://github.com/EffortlessMetrics/ripr-swarm/issues/1977) - bind
  analysis and verification artifacts to repository, revision, schema, and
  content identities.
- [#1941](https://github.com/EffortlessMetrics/ripr-swarm/issues/1941) - agent
  verify and receipt trust boundary.

Support-tier impact:

- No support-tier promotion. This contract makes static artifact currentness
  and integrity explicit; it does not claim runtime verification, correctness,
  mutation results, or merge authority.
- Claim boundaries remain governed by the [support tiers](../status/SUPPORT_TIERS.md).

## Problem

Saved snapshots previously reached `agent verify` when they merely contained a
parseable `seams` or `findings` array. That allowed fabricated or stale input to
be treated as movement evidence.

## Behavior

`ripr check --format repo-exposure-json` emits an additive top-level `artifact`
identity envelope. The envelope has `kind = "repo_exposure"`, identity schema
`"1"`, producer/tool version, analyzed root, Git HEAD when available, format,
mode, base revision, worktree state, bounded analysis-input identity,
snapshot identity, creation command/profile, and `content_sha256`.

The analysis-input identity covers the selected root, base, mode, named
workspace inputs, and analyzer version without exposing configuration bytes.
The content commitment uses the `raw_json_placeholder_v1` rule: hash the exact
artifact bytes after replacing the one `content_sha256` value with the fixed
zero digest placeholder. The producer emits the resulting digest in the final
artifact. This rule is stable, bounded-memory, and detects later byte changes.

`ripr agent verify` accepts only repo-exposure artifacts with this envelope. It
rejects missing or unsupported identity, root mismatch, invalid HEAD,
malformed or duplicate commitments, content mismatch, and incomparable base or
analysis-input identities before movement calculation. It adds
`artifact_currentness` to its
advisory output with one of:

- `current`;
- `dirty_worktree`;
- `historical_noncurrent`.

When Git identity is unavailable, the producer discloses `unavailable` in the
artifact and the verifier rejects it as unsuitable evidence.

## Non-claims

- The envelope is not a digital signature or proof against a compromised RIPR
  process.
- `agent verify` still compares static before/after evidence only; it does not
  execute tests or runtime mutation testing.
- Receipt issuance, command execution, configuration binding, and replay
  protection remain follow-up slices under #1941.

## Non-Goals

- No runtime test or mutation execution.
- No receipt signature, remote attestation, or merge-policy change.
- No configuration or command-execution binding beyond the producer metadata
  recorded here.

## Required Evidence

- Producer output tests cover identity and streaming output.
- CLI smoke tests cover a valid bound pair, a historical comparable pair,
  dirty-worktree disclosure, tampered bytes, incomparable input identities,
  unsupported schema, malformed typed seam, and plausible uncommitted JSON.
- The editor repair-loop fixture consumes bound artifacts and records explicit
  currentness.

## Acceptance Examples

### Current bound pair

Two snapshots from the same root with the same base identity and current HEAD
produce advisory movement with `artifact_currentness = "current"`.

### Tampered or fabricated input

Changing any byte after emission, omitting the `artifact` envelope, or supplying
an unsupported schema fails before movement calculation.

## Test Mapping

- `crates/ripr/src/agent/artifact.rs` tests the fixed commitment protocol and
  duplicate-field rejection.
- `crates/ripr/tests/cli_smoke.rs` tests valid, tampered, fabricated, and
  editor-loop cases.

## Implementation Mapping

- `crates/ripr/src/agent/artifact.rs` owns identity and commitment validation.
- `crates/ripr/src/output/repo_exposure.rs` emits the bounded two-pass artifact.
- `crates/ripr/src/cli/commands.rs` validates both inputs before movement.
- `crates/ripr/src/output/outcome/render_json.rs` discloses currentness.

## Metrics

- `repair_artifact_provenance_status_accepted` records that this first
  provenance slice is available.
- Future slices under #1941 must add execution, configuration, and receipt
  currentness metrics without reusing this field as runtime proof.
