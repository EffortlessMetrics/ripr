---
name: prepare-proof
description: Design discriminating proof for one selected claim before or during implementation. Use when tests are absent, weak, self-confirming, or disconnected from the production path.
---

# Useful result

A bounded proof contract can distinguish the intended behavior from the strongest plausible wrong implementation and reaches the real production authority.

# Procedure

1. Name the exact claim, producer, consumer, and authority boundary.
2. Inspect current tests, fixtures, goldens, schemas, reports, and CI routes before adding another oracle.
3. Describe the cheapest positive case that exercises the retained behavior.
4. Design at least one discriminating negative or alternate case that would pass under a fake, stale, proxy, or nearby implementation.
5. Prove production-path reachability:
   - the test invokes the public or retained entry point;
   - the relevant branch or owner actually runs;
   - fixture setup succeeds before downstream assertions;
   - parsed inputs contain the intended subject;
   - skipped or unobserved work is not treated as pass.
6. Separate source failure, test-or-oracle failure, instrument failure, infrastructure failure, and not-established states.
7. Prefer a removal or wrong-implementation experiment where it materially strengthens the oracle.
8. Record expected artifacts, identities, currentness, and claim limits.
9. Review the proof against the strongest counter-read before implementation continues.

# Proof laws

- Output presence is not behavioral proof.
- A golden can encode an incorrect expectation; use an independent corpus or invariant where promotion risk is material.
- A setup helper must fail at setup rather than manufacture a downstream product failure.
- A status-zero command with missing or unrecognized evidence is not clean.
- Platform-specific branches require platform-capable proof.
- Process success, static movement, mutation adequacy, and merge readiness remain separate axes.

# Useful fan-out

A read-only test-oracle reviewer may challenge fake oracles, token coincidence, path/platform assumptions, and missing negative cases. A production-path reviewer may independently map the actual consumer. The root reconciles one proof contract.

# Valid exits

- `PROOF_READY`
- `PROOF_NEEDS_REPAIR`
- `PRODUCTION_PATH_NOT_REACHED`
- `INSTRUMENT_FAILURE`
- `EXTERNAL_BLOCKER`
- `NOT_ESTABLISHED`
