# RIPR-SPEC-0126: LSP diagnostic delivery budget

Status: accepted

This specification defines the pure, deterministic delivery-budget projection
for passive LSP diagnostics. It is the PR-A contract for issue #1582.

## Problem

The budget runs after producer-owned canonical identity, profile eligibility,
actionability, causal attribution, and evidence readiness have been established.
It does not create, merge, split, strengthen, weaken, renumber, or delete
canonical evidence. It does not change gate authority.

The selector receives:

- a stable canonical identity;
- a stable document identity;
- the UTF-8 byte size of the normalized passive payload;
- optional inline-detail byte size;
- producer-owned eligibility;
- producer-owned repair-route, causal, and evidence-order ranks.

The selector never infers product or business severity from names, paths,
visibility, crate names, prose, line numbers, traversal order, wall-clock
time, or hash-map order.

## Behavior

The schema version is 'lsp-diagnostic-budget-v1'. The selection-order version is
'evidence-order-v1'. A budget contains positive finite limits for:

- items per document;
- items per workspace response;
- serialized passive-payload bytes;
- inline detail bytes.

Contradictory or zero limits are rejected. A caller must choose an explicit
safe fallback; this pure evaluator never silently disables bounding.

The result contains:

- snapshot/profile/budget identity;
- complete-evidence identity;
- continuation or inspect route;
- total canonical and profile-eligible counts;
- selected canonical items and exact serialized bytes;
- every omitted canonical identity and omission reason;
- complete eligible bytes where measured;
- overflow state and the set of active overflow reasons.

Profile-filtered items are recorded as 'profile_filtered' but do not consume the
passive delivery budget. Eligible items omitted by a finite limit are recorded
as 'document_item_limit', 'workspace_item_limit', or 'serialized_byte_limit'.

## Selection

Eligible items are traversed exactly once in this producer-owned order:

1. repair-route rank;
2. causal rank;
3. evidence rank;
4. canonical identity;
5. document identity.

Lower rank values are selected first. The final identities make ties stable.
Per-document and workspace item limits are applied before serialized-byte
limits. If the first eligible item alone exceeds the byte budget, it remains
selected so the canonical item is never silently hidden; the result remains
explicitly overflowed.

Inline detail that exceeds its limit does not remove its canonical item. The
item remains selected with 'inline_detail_omitted', and a later typed retrieval
surface can provide the complete detail.

The selected set is a bounded working view. It is not a product-risk ranking
and does not imply that omitted items are less important.

## Required invariants

- Equal inputs in any discovery order produce equal results and bytes.
- Adding a lower-ranked item does not reshuffle existing selected identities.
- Every canonical input is present in exactly one selected or omitted result.
- Profile-filtered items never consume an item or byte budget.
- Overflow is machine-readable and cannot be presented as a complete inventory.
- Complete evidence identity is independent of the passive budget.
- Transport, pull/push, status, client, and gate policy are out of scope for PR A.

## Follow-up contracts

PR B applies this evaluator to LSP status and push projection. PR C provides
pull parity and result identity. PR D provides stateless complete-evidence
continuation and stale-continuation errors. Those layers must consume this
result without deriving a second ordering or canonicalization rule.

## Required Evidence

The PR-A proof must show that the evaluator accounts for every canonical input,
records exact omission reasons, keeps profile-filtered items outside the budget,
and produces the same result for equivalent input orderings.

## Non-Goals

- Applying the selector to live LSP push or pull transport.
- Implementing continuation storage or client UI.
- Re-ranking canonical evidence by inferred product or business severity.
- Changing gate, actionability, causal-attribution, or canonicalization authority.

## Acceptance Examples

- A document cap omits later eligible identities with `document_item_limit`.
- A workspace cap omits later eligible identities with `workspace_item_limit`.
- A byte cap omits later eligible identities with `serialized_byte_limit`.
- Oversized inline detail keeps its canonical item and reports
  `inline_detail_omitted`.
- Profile-filtered items are recorded but never consume a delivery slot.
- Equivalent input order produces byte-equivalent selected and omitted identity
  order.

## Test Mapping

The pure fixtures live in
`crates/ripr/src/lsp/diagnostic_budget.rs::tests` and cover deterministic
ordering, document/workspace/byte limits, oversized detail, profile filtering,
and invalid budgets.

## Implementation Mapping

- `crates/ripr/src/lsp/diagnostic_budget.rs` — versioned budget types and pure
  selector.
- `crates/ripr/src/lsp.rs` — internal module registration.

## Metrics

PR B owns runtime emission of these reserved metrics when the evaluator is
connected to LSP delivery:

- `lsp_diagnostic_budget_selected_items`;
- `lsp_diagnostic_budget_omitted_items`;
- `lsp_diagnostic_budget_overflowed`.

PR A defines their names and does not emit them yet.

## Proof

The PR-A fixture suite covers deterministic ordering, per-document and
workspace limits, serialized-byte overflow, oversized inline detail,
profile-filtered items, and invalid budgets.
