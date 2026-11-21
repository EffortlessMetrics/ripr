# RIPR-SPEC-0125: Rust missing-discriminator evidence fixtures

Status: proposed

## Problem

Pin the minimized producer limitation exposed by the governed Rust pilot and
keep the four exact real-repository observations available to the next
producer implementation slice. This specification is an evidence and
regression contract, not a support or repair-readiness claim.

## Behavior

- The minimized control changes an error variant while its related test uses a
  broad `is_err()` oracle.
- The expected static result retains the exact changed expression, flow sink,
  broad oracle, and named missing discriminator.
- Each pilot mapping identifies one repository, source PR, exact analyzed
  head, canonical candidate, and evidence reference.
- Mappings do not count governed attempts, authorize source edits, or promote
  synthetic fixtures into the real-repository corpus.
- Producer work may replace the generic limitation only when it supplies an
  exact discriminator attached to the canonical owner and behavior identity.

## Required Evidence

- The minimized fixture produces the named missing-discriminator evidence and
  broad-oracle facts in its golden output.
- Each of the four pilot mappings retains its exact repository, source PR,
  analyzed head, canonical candidate, and evidence reference.
- Fixture execution and fixture-contract validation pass without modifying a
  governed adopting repository.

## Non-Goals

- No renderer, LSP, packet, gate, or review-comment fallback.
- No inferred target test, assertion, verify command, or receipt command.
- No runtime mutation result or support-tier promotion.

## Acceptance Examples

1. The broad-error control remains weakly exposed and names the exact error
   variant as missing evidence.
2. The four pilot mappings remain four distinct observations, even when they
   point at the same minimized control.
3. A later producer change can update the expected limitation only with a
   fixture-backed reason and a corresponding real-repository receipt.

## Test Mapping

- `cargo xtask fixtures rust_missing_discriminator_evidence` executes the
  minimized control and compares its static output to the checked-in golden.
- `cargo xtask check-fixture-contracts` validates the BDD fixture shape.
- `pilot-mappings.json` is the exact-head evidence inventory for #1601 PR2.

## Claim boundary

This contract proves only the minimized static limitation and preserves exact
pilot bookkeeping. It does not prove analyzer completeness, route readiness,
repair usefulness, runtime mutation outcomes, or support promotion.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `fixtures/rust_missing_discriminator_evidence/diff.patch` | minimized broad-oracle control |
| `fixtures/rust_missing_discriminator_evidence/expected/check.json` | static JSON golden |
| `fixtures/rust_missing_discriminator_evidence/pilot-mappings.json` | four exact pilot head mappings |
| `docs/dogfood/2026-07-13-rust-pilot.md` | real-pilot observation authority |
| `metrics/rust-repair-trust/corpus.json` | governed corpus counting authority |

## Metrics

- `rust_missing_discriminator_fixture_cases`
- `rust_missing_discriminator_pilot_mappings`
- `rust_missing_discriminator_real_eligible_attempts`

## Proof

```text
cargo xtask fixtures rust_missing_discriminator_evidence
cargo xtask check-fixture-contracts
```

The real-repository evidence authority remains
`docs/dogfood/2026-07-13-rust-pilot.md` and
`metrics/rust-repair-trust/corpus.json`.
