# B5: provider-contract corpus benchmark

A small JSON corpus over the exact-snapshot provider DTOs in
`crates/ripr/src/provider_contract/` (`model.rs`, `validate.rs`).
Each invalid case is exactly one mutation away from a valid baseline,
so a failure names the single rejected field.

## Layout

- `valid-request.json`: baseline `RiprAnalysisRequestV1` (git-tree
  identity `0123456789abcdef0123456789abcdef01234567` with its
  matching `source_digest`).
- `valid-receipt.json`: baseline `RiprAnalysisReceiptV1` (completed,
  canonical exposure status, evidence summary).
- `valid-capabilities.json`: baseline `RiprProviderCapabilitySetV1`
  (single read-only, offline, non-executing descriptor).
- `corpus.json`: bases plus cases. Each case names its base, its
  mutations (JSON-pointer sets/removes/appends), and the expected
  outcome: `Ok` or one `RiprProviderContractErrorCodeV1` variant.
- Harness: `crates/ripr/tests/agentic_bench_provider.rs`.

## Oracle

- git-tree identity: 40/64 lowercase hex plus a `source_digest`
  that is the SHA-256 of the canonical `snapshot_id`
  (`MalformedIdentity` / `IdentityMismatch`);
- authority: any write/execute/network descriptor bit is
  `AuthorityViolation`;
- completeness: truncated-yet-complete, status- or summary-carrying
  non-authoritative results, and inconsistent summary denominators
  are `CompletenessConflict`.
