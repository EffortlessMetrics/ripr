# Review-comment reasons follow canonical card state

Owning implementation issue: [#4912](https://github.com/EffortlessMetrics/ripr-swarm/issues/4912). Immutable starting main: `0dd150619632394437f3c167d274c61ef745bbf0`.

## Problem and authority

The real #1580 replay recorded in [#3074](https://github.com/EffortlessMetrics/ripr-swarm/issues/3074#issuecomment-5888783137) retained static-limitation cards whose `reason` encouraged a focused test while their typed guidance refused repair edits. This is a copy projection defect, not evidence of a repair route. Parent: #1693. Shared projection authority: #1663 and #1895.

## Narrow implementation

In `crates/ripr/src/output/review_comments.rs`, pass the already-derived canonical `gap_state` into `reason_for`. Only `actionable` may retain missing-discriminator or focused-test language. Other states must project their actual state without suggesting an edit. Do not derive readiness from missing values or prose. Preserve schema, typed guidance, identities, limits, commands and selection.

## Acceptance and proof

- Render the real existing static-limitation fixture and reject test-strengthening copy while retaining its typed refusal.
- Cover state precedence even when an optional missing discriminator exists.
- Preserve actionable missing-discriminator and actionable fallback reasons.
- Cover already-observed, internal-only and unknown states with no repair invitation.
- Use fallible regression assertions; no new source exception.

Proof: `cargo test -p ripr --lib review_comments -- --nocapture`, owning output-contract checks, `git diff --check`, and scoped rustfmt. Native inherited baseline and runtime proof are **not run** while the shared target lease belongs to another lane; source preparation is explicitly authorized, not runtime qualification. Root substantive source review precedes draft publication.

## Material prerequisite integration and proof custody

The original0cb candidate compiled and passed both named controls1/0/0 each and all51 review-comment module tests. That evidence belongs to0cb, not its successor. Raw/context/executable/liveness were independently counterread by root; shared lease released. No overall guard qualification was claimed.

#4911 actually merged as67de8927b5d39bd437d33272eda3b4d41b3d567b, repairing known inherited Windows PyPI identity/isolation guard failures. This material prerequisite justified ordinary normal integration (not behind-only restacking). Merge f59393bd5d21ee40660e18619cc5392aa8fa1366 has clean combined tree672903b1cd7a577ebb4c414ba37b165f6dd98d8f;161 inherited paths retained in external inventory. Reason projection, canonical gap_state/actionability and typed guidance owner blobs are unchanged from0cb. New upstream classifier boundary-pairing, parser and origin work remains upstream-owned; fresh combined guards must qualify integration. Sharedorigin/main53 was not moved.

Successor compilation/module/output-contract/golden/check-fast/precommit proof is NOT_RUN pending exclusive native lease and root review of the prepared runner. Derive exact selector/renames and current command/gate schema from this committed successor; no old denominator or receipt context may be substituted.

## Non-goals and remaining investigation

No new analyzer facts, test generation, consumer edits, canonical-route claim, or release claim. The #1981/#4833 investigation still needs the actual first refusing binder gate: null cards alone do not establish its cause, and mutable/nonliteral production RHS is not an established refusal. Keep that work with its existing owner. Rollback is this scoped projection change; no data migration.
