# ADR 0019: Language Adapters Reuse the Shared Packet Contract

Status: proposed

Date: 2026-06-13

Artifact ID: RIPR-ADR-0019

## Context

`ripr` answers one draft-time question: for the behavior changed in this diff,
do the current tests contain a discriminator that would notice if that behavior
were wrong? When a finding is a true test-intent gap, `ripr` can hand the agent
an actionable repair packet — a typed, fail-closed unit of action with a repair
route, allowed edit surface, verification commands, and a receipt command.

For the Rust reference adapter, "actionable" already had a single home. The
`repair_packet_ready` flip is owned by exactly one authority,
`validate_agent_gap_record_packet` in
[`crates/ripr/src/output/agent_seam_packets.rs`](../../crates/ripr/src/output/agent_seam_packets.rs)
(declared at line 839), and the rendering surfaces consume shared agent-seam
helpers — `allowed_edit_surface_for_gap_route` (line 906) and
`gap_record_packet_do_not_do` (line 1257) — that read from a `GapRecord`
([`crates/ripr/src/output/gap_decision_ledger.rs`](../../crates/ripr/src/output/gap_decision_ledger.rs),
lines 87-134).

Adding TypeScript actionability under
[RIPR-SPEC-0087](../specs/RIPR-SPEC-0087-typescript-preview-actionable-repair-packet.md)
and
[RIPR-SPEC-0088](../specs/RIPR-SPEC-0088-typescript-repair-packet-projection.md)
created a concrete risk: a *second* notion of "actionable." A TypeScript
projection could grow its own inline validator, its own "is this ready" boolean,
and its own renderer — and that parallel notion would drift away from the Rust
one over time, silently. The first error rate (false-actionable) is at least
visible in emitted output; a drifting parallel validator is the kind of failure
that hides until two languages disagree about whether the same shaped gap is
safe to hand an agent.

The fix demonstrated in the TypeScript wave is structural, not procedural:
the TypeScript projection
([`crates/ripr/src/output/typescript_packet_projection.rs`](../../crates/ripr/src/output/typescript_packet_projection.rs))
builds a `GapRecord` via `typescript_gap_record_for` and flips
`repair_packet_ready` only when the *shared* validator passes
(`validate_agent_gap_record_packet(record).is_ok()`, computed in
[`crates/ripr/src/output/preview_actionability.rs`](../../crates/ripr/src/output/preview_actionability.rs),
lines 66-68). It renders through the shared agent-seam helpers, and it carries a
`validator_parity_*` test suite
(`typescript_packet_projection.rs`, lines 382-494) that holds the TypeScript
projection and the shared Rust validator in lockstep.

This ADR records that pattern as the standing constraint for *every* current and
future language adapter, so the next adapter author does not re-litigate it or
quietly grow a mirror.

## Decision

Any language adapter that emits actionable repair packets MUST:

1. **Build a `GapRecord`.** The adapter projects its language-native finding into
   the shared `GapRecord` container (`gap_decision_ledger.rs`, lines 87-134),
   following the `typescript_gap_record_for` pattern. Language-native parser
   types stay inside the adapter; the packet contract is language-neutral.

2. **Pass the shared `validate_agent_gap_record_packet`.** That function
   (`agent_seam_packets.rs`, line 839) is the *single* authority for the
   `repair_packet_ready` flip. The flip is fail-closed: an absent or ineligible
   `GapRecord` (an `Option::None` projection) yields `repair_packet_ready:
   false`. A packet is "actionable" only when this shared validator returns
   `Ok`.

3. **Render via the shared agent-seam helpers.** Edit surface comes from
   `allowed_edit_surface_for_gap_route` (line 906); the "do not edit" guidance
   comes from `gap_record_packet_do_not_do` (line 1257). All surfaces that
   present the packet — human sections, JSON report, LSP hover, LSP actions —
   consume these helpers rather than formatting their own.

4. **Carry a parity test.** A `validator_parity_*`-style suite (the TypeScript
   reference is `typescript_packet_projection.rs`, lines 382-494) MUST hold the
   adapter's projection and the shared validator in agreement: a complete
   finding projects and passes; each missing or disqualifying field returns
   `None` / fails closed.

An adapter MUST NOT introduce a parallel, mirror, or inline validator; a
language-local `repair_packet_ready` boolean; or a bespoke packet renderer. There
is one validator and one set of agent-seam render helpers, shared across
languages.

## Consequences

Positive:

- **One notion of "actionable" across Rust, TypeScript, and future languages.**
  Because every adapter routes its flip through the same
  `validate_agent_gap_record_packet`, drift between languages is impossible by
  construction, not merely discouraged by review.
- **The parity test is the enforcement mechanism.** Each adapter's
  `validator_parity_*` suite fails the build the moment a projection and the
  shared validator disagree, so a drifting mirror cannot land silently.
- **The fail-closed default carries over for free.** `Option<GapRecord> -> None
  -> repair_packet_ready: false` holds for every adapter; new languages inherit
  the conservative posture without re-deriving it.
- **Render reuse keeps the agent-facing packet uniform** — same edit-surface and
  do-not-edit semantics regardless of source language.

Negative / costs:

- Each new adapter pays a fixed cost: build the `GapRecord`, wire the shared
  validator, route through the shared helpers, and author the parity suite. This
  is deliberate — it is cheaper than reconciling two divergent notions of
  "actionable" later.
- The shared `GapRecord` must remain language-neutral. If a language genuinely
  needs a field the container does not model, the field is added to the shared
  contract (and re-blessed across goldens) rather than forked into a
  language-local validator.
- **Derived messaging must be reconciled in the layer that owns the final
  decision.** Surfaces relabel "why not actionable" to "why actionable" only
  *after* the shared validator flips `repair_packet_ready`
  (`preview_actionability.rs`, lines 60-125; human sections at
  `crates/ripr/src/output/human/sections.rs`, lines 167-209). Display strings,
  category labels, and `evidence_needed` are derived from the validated record —
  they must not be computed independently per surface, or the message can
  disagree with the typed decision.

## Alternatives Considered

| Alternative | Why rejected |
| --- | --- |
| Let each adapter own its own validator and `repair_packet_ready` boolean. | Maximizes local freedom but guarantees two (then three) drifting notions of "actionable"; cross-language disagreement is silent and undetectable from output. |
| Share the renderer but allow a thin per-language "readiness" check. | Any second readiness check is a parallel validator by another name; it re-opens the drift surface the parity test exists to close. |
| Enforce the single-validator rule by review/convention only. | Conventions do not fail the build. The parity test makes the constraint mechanical (per [ADR 0003](0003-fixtures-before-analyzer-rewrites.md): evidence before behavior). |
| Reconcile derived messaging independently on each surface. | Per-surface message computation lets the human label drift from the typed decision; messaging must be derived in the layer owning the flip (per [ADR 0015](0015-start-here-surfaces-use-canonical-gap-records.md): typed fields are authoritative, prose is display). |

## Non-goals

- This ADR does not change analyzer behavior, the static exposure model, or the
  conservative static language ([ADR 0002](0002-static-exposure-language.md)).
- This ADR does not promote any preview-language evidence; preview promotion
  remains policy-owned, and RIPR-SPEC-0087 / RIPR-SPEC-0088 remain `proposed`.
- This ADR does not add a new output schema; it constrains *how* adapters reach
  the existing packet contract.
- This ADR does not mandate which parser substrate an adapter uses — that stays
  per-language (see [ADR 0008](0008-typescript-parser-substrate.md),
  [ADR 0009](0009-python-parser-substrate.md),
  [ADR 0018](0018-perl-lsp-fact-substrate.md)).

## Related Specs and ADRs

- [RIPR-SPEC-0087: TypeScript preview actionable repair packet](../specs/RIPR-SPEC-0087-typescript-preview-actionable-repair-packet.md) (proposed) — defines the shared-validator flip authority.
- [RIPR-SPEC-0088: TypeScript repair packet projection](../specs/RIPR-SPEC-0088-typescript-repair-packet-projection.md) (proposed) — four-surface projection reusing shared helpers.
- [ADR 0015: Start-Here Surfaces Use Canonical Gap Records](0015-start-here-surfaces-use-canonical-gap-records.md) — typed records are authoritative; prose is display.
- [ADR 0008](0008-typescript-parser-substrate.md) / [ADR 0009](0009-python-parser-substrate.md) / [ADR 0018](0018-perl-lsp-fact-substrate.md) — per-language substrates; the packet contract is shared across all of them.
