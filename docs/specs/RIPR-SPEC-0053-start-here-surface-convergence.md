# RIPR-SPEC-0053: Start-Here Surface Convergence

Status: accepted

Accepted: 2026-05-22 after the Start-Here Surface Convergence closeout
recorded PR/CI, CLI, receipt lifecycle, no-output/fail-closed, preview
promotion-criteria, dogfood, and documentation proof while preserving the
advisory, static-only, no-source-edit, no-generated-test, no-provider, and
no-mutation boundary.

## Problem

RIPR has multiple useful surfaces: editor cockpit, CLI, generated CI, first-pr
packet, PR evidence, receipts, report packet indexes, preview-language output,
and release/install docs. The user should not need to understand each artifact
graph to know what to do next.

This spec defines the shared first-screen contract for surfaces that claim to
guide a user toward one safe repair.

## Behavior

Start-here surfaces should present the same repair-oriented shape when the
underlying artifacts can support it. A surface may be CLI output, generated CI
summary text, PR evidence, first-pr packet, report packet index, receipt
summary, editor projection, or user documentation.

The behavior is additive and advisory unless a separate policy/gate artifact
owns pass/fail authority.

### Canonical Start-Here Unit

When a surface has an actionable gap, it should lead with typed fields when
available:

```text
canonical_gap_id or gap_id
language
language_status
gap_state
repair_route
related_test.path
verify_command
receipt_command
receipt_state
static_limit_kind or static-limit text
policy_overlay
artifact_freshness
non_claims
```

Raw findings, counts, evidence snippets, static labels, and confidence wording
are supporting evidence. They must not be the primary work item when a
canonical gap record or repair route exists.

### PR/CI first screen

Generated summaries and PR evidence should answer:

```text
What is the one repairable gap?
Why does it matter?
Where should the focused test go?
What command verifies movement?
What receipt proves movement?
What remains limited or advisory?
```

They must not imply gate pass, merge approval, runtime adequacy, or mutation
proof unless a separate gate or runtime artifact explicitly carries that
authority.

### CLI front door

`doctor`, `first-pr`, `pr-ready`, cockpit reports, and related helper commands
should use the same state names and next-action language:

```text
start here
safe next action
missing artifact
stale evidence
wrong root
malformed artifact
no actionable gap
preview-limited evidence
verify command
receipt command
receipt path
```

### Receipt lifecycle

Surfaces that show receipt state should distinguish:

```text
receipt_missing
receipt_found
receipt_stale
receipt_gap_mismatch
receipt_movement_improved
receipt_movement_unchanged
receipt_not_applicable
```

A receipt is static proof of artifact movement. It is not runtime adequacy,
mutation proof, merge approval, or policy eligibility by itself.

### No-output and fail-closed states

Surfaces should not collapse "nothing happened" into empty output. They should
name the most specific known state:

```text
clean
no_actionable_gap
missing_artifacts
stale_evidence
wrong_root
language_disabled
adapter_unavailable
preview_disabled
preview_limited
malformed_artifact
timeout_partial
server_unavailable
unsupported_schema
unsafe_path
unsafe_command
```

Unsafe states may provide refresh, regeneration, setup diagnosis, or manual
inspection guidance, but they must suppress stronger repair claims.

### Preview-language promotion

TypeScript, JavaScript, and Python remain preview until a policy-owned
promotion packet says otherwise. Promotion criteria must include:

- fixture matrix for emitted evidence classes;
- real-repo or external-style dogfood receipts;
- related-test accuracy review;
- static-limit taxonomy coverage;
- false-positive and false-repair-packet review;
- PR/CI, CLI, and editor consistency;
- policy-readiness signoff.

Routing, parser compilation, or fixture existence alone is not promotion.

Promotion criteria are owned by policy surfaces, not by the editor, generated
CI, or the language adapters. A start-here surface may show preview evidence,
but it must keep the language status visible and must not turn advisory routing
into a repair, gate, or merge decision.

Required proof before a preview language or adapter-family class can even be
reviewed for stronger status:

| Proof area | Required evidence | Failure mode if missing |
| --- | --- | --- |
| Fixture matrix | The candidate language/class has fixtures for emitted evidence, no-output states, static limits, malformed inputs, and disabled or unavailable adapter states. | Keep the evidence preview/advisory. |
| Dogfood receipts | External-style receipts exercise the start-here loop on a normal repository shape and at least one fail-closed state. | Do not claim the loop is adopter-ready. |
| Related-test accuracy | Maintainer-reviewed samples show related-test routing does not send repair packets to the wrong proof surface. | Suppress stronger repair authority. |
| Static-limit taxonomy | Known parser, adapter, import-graph, dynamic dispatch, and unsupported-syntax limits are labeled or excluded. | Keep static-limit states visible before action language. |
| False-positive review | Maintainer-reviewed samples document over-reporting risk for the narrow class. | Do not use the class for policy eligibility. |
| False repair packet review | Maintainer-reviewed samples show preview packets do not invent safe repairs or overstate confidence. | Do not expose stable repair authority. |
| Surface consistency | PR/CI, CLI, editor, receipts, docs, and report packets show the same preview/advisory boundary. | Treat inconsistent surfaces as promotion blockers. |
| Policy signoff | A policy owner signs off on the narrow language/class and rollback path. | No promotion review can close. |

JavaScript evidence that flows through the TypeScript preview adapter family is
still JavaScript preview evidence. A TypeScript packet does not automatically
promote JavaScript unless a later policy-owned packet explicitly names that
scope.

## Required Evidence

Follow-up implementation PRs should provide evidence appropriate to the changed
surface:

- output-contract tests for generated CI, PR evidence, or first-pr packet
  changes;
- CLI smoke or unit tests for command text and state-name changes;
- receipt fixture or dogfood receipts for receipt lifecycle changes;
- support-tier, policy, or promotion-packet docs for preview-language
  promotion criteria;
- external-style dogfood receipts for adoption-path claims;
- traceability entries linking the spec to tests, outputs, and metrics as each
  surface lands.

## Non-Goals

- No analyzer behavior changes by default.
- No output schema changes without a scoped spec-test-code PR.
- No generated CI blocking or default gate behavior changes.
- No PR comment publishing changes.
- No preview-language promotion without a policy-owned promotion packet.
- No source edits, generated tests, provider/model calls, mutation execution,
  CodeLens, inlays, semantic tokens, inline patches, or unsaved-buffer
  overlays.

## Non-Claims

Start-here surfaces must not claim:

- tests are adequate;
- mutation testing ran;
- runtime behavior is proven;
- coverage is sufficient;
- a PR is merge-approved;
- a gate passed unless a gate artifact says so;
- preview evidence is Rust-level confidence;
- RIPR edited source files or generated tests.

## Acceptance Examples

Actionable Rust gap:

- PR/CI, CLI, and editor surfaces name the same gap identity, repair route,
  verify command, receipt command, and receipt state.

Preview Python top gap:

- Start-here surfaces may select a Python preview gap from explicit gap-ledger
  evidence, but must show `language_status = "preview"`,
  `output_state = "preview_limited"`, static-limit/advisory boundary, verify
  command, and receipt command before any repair guidance is treated as a work
  order.

No actionable gap:

- Surfaces report no actionable gap without implying coverage adequacy,
  mutation adequacy, runtime proof, or merge readiness.

Stale evidence:

- Surfaces report stale evidence, point to refresh/regeneration, and suppress
  repair actions that require current evidence.

Preview static limit:

- Surfaces show preview language status and static limits before suggested
  action language.

Receipt improved:

- Surfaces show that the receipt records improved static movement while keeping
  runtime and gate non-claims visible.

## Implementation Mapping

Planned slices:

1. `docs(product): open start-here surface convergence stack`
2. `report: align PR/CI first screen on canonical repair unit`
3. `cli: converge start-here command language`
4. `receipt: standardize receipt lifecycle state`
5. `output: standardize no-output and fail-closed states`
6. `policy(language): define preview promotion proof criteria`
7. `dogfood: record external-style start-here receipts`
8. `campaign: close start-here surface convergence`

Each slice should add tests, fixtures, output contracts, metrics, or docs only
for the surface it changes.

## Test Mapping

Expected test and proof surfaces as the burn-down lands:

- `crates/ripr/src/output/*` tests for report and PR/CI first-screen changes;
- `crates/ripr/src/cli/*` tests and `crates/ripr/tests/cli_smoke.rs` for CLI
  command-language changes;
- receipt-related output tests and dogfood receipt checks for lifecycle state;
- policy promotion tests or docs checks for preview promotion criteria;
- `cargo xtask dogfood` and committed handoff receipts for external-style
  adoption proof;
- `cargo xtask check-output-contracts`, `cargo xtask check-traceability`, and
  `cargo xtask check-pr` for review readiness.

## Metrics

Candidate metrics for follow-up PRs:

- `start_here_surfaces_with_canonical_gap`;
- `start_here_surfaces_with_receipt_state`;
- `start_here_fail_closed_states`;
- `start_here_preview_promotion_blockers`;
- `start_here_external_dogfood_receipts`.

Metrics should be added only when backed by code, tests, and traceability.
