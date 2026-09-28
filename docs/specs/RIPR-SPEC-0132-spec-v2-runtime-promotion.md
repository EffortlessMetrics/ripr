# RIPR-SPEC-0132: Spec-v2 runtime-promotion boundary

Status: proposed

Owner: product / swarm

Created: 2026-07-15

Linked proposal:

- None yet

Linked ADRs:

- None yet

Linked plan:

- None yet

Linked issues:

- #1667
- #1672
- #1676

Linked PRs:

- #1691

Support-tier impact:

- None

This specification introduces a repository-control-plane requirement and
promotes no runtime, language, editor, proof, or release surface.

Policy impact:

- Register this proposed specification in the RIPR and cargo-allow document
  ledgers.
- Add one compact PR-local implementation slice under `.allow/spec-system/slices/`.
- No hook, workflow, branch-protection, test-execution, or support policy
  changes in this slice.

## Problem

RIPR specifications currently express useful behavior at document level, but a
behavior PR cannot identify the exact normative requirement generation it
implements. A specification-only change can therefore look indistinguishable
from runtime implementation, current proof, or support promotion in downstream
prose and generated views.

The first RIPR-SPEC v2 requirement must establish the smallest fail-closed
boundary needed to keep those states separate before the broader requirement,
evidence, and receipt graph lands.

## Normative Requirements

```toml cargo-allow-requirements
schema_version = "1.0"

[[requirement]]
id = "spec-only-runtime-promotion"
generation = 1
lifecycle = "accepted"
statement = "A spec-or-governance-only implementation slice may define or amend a runtime requirement, but it cannot mark runtime behavior implemented, claim current runtime proof, or promote runtime support without compatible implementation and evidence dispositions."
claim_class = "runtime_behavior"
```

## Behavior

A `SpecOrPolicyChange` or repository-equivalent spec/governance slice may:

- introduce or amend an accepted runtime requirement;
- name planned implementation seams and evidence obligations;
- state that implementation and proof remain outstanding;
- leave support claims unchanged.

It must not, without compatible implementation and evidence dispositions:

- transition a runtime requirement to `Implemented`;
- claim that runtime proof is current;
- promote a runtime support or release claim;
- allow a generated index, issue state, active goal, or merged specification to
  strengthen authored runtime state.

Rejected promotion must be atomic: an invalid slice produces no accepted
runtime or support transition.

## PR-local implementation slice

The source for this proposed change is
`.allow/spec-system/slices/spec-v2-runtime-promotion.v1.toml`.

That slice is classified as a spec/governance change. It names this requirement
generation and explicitly records:

- runtime implementation outstanding;
- runtime evidence outstanding;
- support claims unchanged;
- the allowed specification and slice surfaces;
- active-goal and support-tier surfaces as forbidden for this PR.

Mutable worker, branch, PR number, CI, review, priority, timing, and progress
state do not belong in the normative slice.

## Required Evidence

This source-only PR requires structural proof that:

- the RIPR document parses through the cargo-allow RIPR-SPEC v2 dialect;
- the compact implementation slice parses through the shared
  `ImplementationSliceV1` model;
- the shared runtime-promotion validator accepts the spec/governance posture
  while leaving runtime implementation, proof, and support outstanding;
- existing RIPR spec numbering, document index, artifact-ledger, and legacy
  traceability checks remain coherent.

Exact runtime validator tests, requirement-to-test evidence edges, RIPR test-grip
summaries, focused execution, and exact-head receipts land in later walking-
skeleton slices. Their absence here is explicit rather than represented as
successful proof.

## Accepted States

- The proposed document defines the requirement while the implementation slice
  keeps runtime implementation and evidence outstanding.
- A later behavior slice may implement the requirement only with compatible
  implementation and evidence dispositions.

## Rejected States

- This source-only slice marks the runtime requirement implemented.
- This source-only slice names current runtime proof without a compatible
  receipt-backed evidence disposition.
- This source-only slice promotes any runtime support claim.
- Generated or legacy compatibility output overrides the authored requirement or
  slice state.

## Acceptance Examples

### Accepted: proposed source with runtime work outstanding

The requirement is `Accepted`; the implementation slice remains
`SpecOrPolicyChange`; implementation and evidence are `outstanding`; and support
is `unchanged`. Structural validation accepts the source without claiming that
runtime behavior exists.

### Rejected: source-only runtime promotion

The same slice changes the runtime requirement to `Implemented`, claims current
proof without a compatible receipt-backed evidence disposition, or promotes a
runtime support claim. Shared validation returns the exact promotion finding and
produces no accepted transition.

### Rejected: generated state strengthens authored state

A generated index, issue state, active goal, or legacy traceability projection
reports a stronger implementation or support posture than the authored
requirement and slice. The authored sources remain authoritative and the
stronger projection is rejected or reported as inconsistent.

## Test Mapping

This source-only PR maps no runtime test as satisfying the requirement:

- the legacy compatibility row names only structural xtask tests
  (`implementation_slices_validate_and_coexist`,
  `spec_system_profile_is_current_v2_without_goal_root`) that prove slice and
  profile shape; they are not runtime evidence for the requirement;
- cargo-allow #2285 supplies shared parser/validator owner tests for the portable
  model, but those external tests do not make RIPR runtime behavior implemented;
- RIPR issue #1677 owns the local exact positive, exact negative, and deliberately
  weak neighboring tests;
- RIPR issues #1678–#1682 own precise evidence edges, test-grip comparison,
  focused execution, and exact-head receipts.

## Implementation Mapping

- `docs/specs/RIPR-SPEC-0132-spec-v2-runtime-promotion.md` — authored document and
  stable requirement generation.
- `.allow/spec-system/slices/spec-v2-runtime-promotion.v1.toml` — compact
  PR-local spec/governance implementation slice (`ImplementationSliceV1`).
- `docs/specs/README.md` — human-readable specification index.
- `.allow/artifacts/doc-artifacts.toml` and `policy/doc-artifacts.toml` —
  cargo-allow and RIPR document registration.
- `.ripr/traceability.toml` — legacy document-level compatibility row with no
  fabricated runtime test or implementation evidence.

No RIPR runtime source is implemented by this PR.

## Metrics

This source-only PR introduces no runtime or product metric. Its review posture
is measured structurally through:

- requirement and implementation-slice parse success;
- spec/index/artifact/traceability consistency;
- zero runtime tests claimed by the legacy compatibility row;
- implementation and evidence remaining outstanding;
- support claim remaining unchanged.

Later dogfood under #1684 owns measured design churn, test-relevance, proof, and
flow metrics.

## Non-Goals

- Implementing the runtime-promotion validator in RIPR.
- Compiling the complete requirement-seam-evidence-test graph.
- Executing runtime proof for this document-only change (running RIPR, cargo tests, mutation tests, or external proof commands). The structural gates required by Required Evidence (format, numbering, index, ledger, traceability, and slice validation) still run and are listed under Proof.
- Migrating existing RIPR specifications or traceability rows.
- Enabling hooks, CI enforcement, branch protection, or support promotion.
- Automatically accepting the requirement outside normal review and merge.

## Claim Boundary

This specification proposes one stable RIPR-SPEC v2 requirement and the source
contract for one compact spec/governance implementation slice. It makes the
runtime-promotion boundary reviewable and machine-addressable. It does not
implement runtime behavior, prove any test adequate, establish current runtime
proof, or promote a support tier.
