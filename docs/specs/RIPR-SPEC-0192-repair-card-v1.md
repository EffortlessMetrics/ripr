# RIPR-SPEC-0192: RepairCardV1 projected from shared repair authorities

Status: proposed

Owner: product / agent

Created: 2026-10-04

Linked issues:

- #4663 (this slice)
- #3166 (parent: repair packet card profile)
- #4666 (bounded detail references and field/budget ratification)
- #4667 (CLI/LSP/MCP projection wiring)
- #4669 (measured agent ratification)

Support-tier impact:

- None. This slice adds a pure projection type and builder; no command
  accepts or executes anything new.
  [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new process, network, or file-policy surface. The card is an
  in-memory DTO built from typed authorities; no renderer or transport is
  added in this slice.

## Problem

The repair packet, the fix-instruction summary, the repair-route readiness
gate, the typed command catalog and the durable repair-attempt manifest each
own one authority. A consumer that needs one compact work order for one
governed repair today has to join those authorities itself, and each consumer
invents its own join — which is exactly how surfaces drift into
re-derivation and claim inflation. #4663 defines the single provider-neutral
card so every renderer or transport can consume one typed object whose
fields name the authority they were copied from.

## Behavior

`crates/ripr/src/domain/repair_card.rs` defines the versioned
`RepairCardV1` DTO, its sub-types, the `REPAIR_CARD_SCHEMA_VERSION`
(`repair_card.v1`) constant, the claim-boundary statement and the single
route-exposure gate. `crates/ripr/src/repair_card_digest.rs` owns the
semantic digest at the crate root — domain must not know JSON rendering,
so the digest logic follows the `command_spec_digest.rs` split. And
`crates/ripr/src/app/repair_card.rs` defines the one builder,
`build_repair_card(&RepairCardInput)`, a pure projection of the existing
authorities:

- `FixInstructionSummary` is embedded verbatim; no instruction meaning is
  re-derived.
- `RepairCardReadinessFacts` preserves `RepairRouteReadiness`'s evidence
  and applies the repair-attempt edit-cage admission as a further ceiling
  on statically ready routes:
  an exact packet refusal closes `repair_ready`, is appended to missing
  evidence, and takes precedence as `exact_blocker`. Static selected-target
  identity remains visible as evidence; it does not authorize an edit.
- `RepairCardTarget` projects `RepairTargetSelection` one-to-one:
  `Existing` keeps the producer-owned symbol identity, file, line, test
  kind, relation basis and portable `workspace_identity`; `Proposed` keeps
  the admitted new-test proposal's file, owner and proposal kind; `Missing`
  stays `None`. Existing and proposed targets can never collapse into one
  variant (`#[serde(tag = "kind")]`).
- The assertion goal is `SuggestedAssertion` when the instruction carries a
  producer-owned suggested assertion, else `ObserverSetup` when the
  producer-owned witness routes through an observer setup; the
  producer-owned detail text rides in `assertion_goal_detail`. The card
  never writes either string itself.
- The next action is a typed `RepairCardCommandRef` (identity, snake_case
  role copied through serde, bounded display); it is a reference to the
  `CommandSpec` authority, never an argv reconstruction from display prose.
- The optional attempt section copies the attempt id and the manifest's own
  snake_case state string; the card never upgrades or completes it.
- `done_when` keeps five separate axes: static movement, focused test
  execution, edit-cage compliance, optional mutation confirmation, and
  snapshot currentness.
- `selected_basis` and a bounded set of load-bearing
  `RepairCardRejectedAlternative`s (at most
  `MAX_REPAIR_CARD_REJECTED_ALTERNATIVES = 3`) record why this target and
  not another; over-boundary input fails closed.

Authority:

1. Route exposure is fail-closed and owned by one gate:
   `repair_card_route_exposable` requires the `FixSiteReady` instruction
   state and the readiness authority's own `is_repair_ready()` flip, and
   the builder additionally requires the producer-owned
   `RepairPacketEligibility` flip and successful edit-cage admission so a cross-language or otherwise
   ineligible seam never receives an actionable route either. A builder
   input that carries a `next_command` while any of those gates is closed
   returns `Err`; stale, limited and unavailable cards can never present a
   current edit or execution route. The edit-cage refusal is supplied by
   `app::repair_attempt::edit_cage_policy_from_packet`, not a second target
   validator in the card or its renderer.
2. Over-boundary `rejected_alternatives` returns `Err`; the card never
   silently truncates load-bearing rejection evidence.
3. The card orders exactly one repair: the subject seam (and canonical gap,
   when both producers name one) must match the readiness authority's own
   identities, and a producer `selected_basis` without a selected target is
   refused; either inconsistency returns `Err` instead of building a card
   that attributes one seam's route to another repair.
4. Semantic identity is a sha256 hex digest over the scoped card surface
   (every load-bearing field except the digest itself). Command `display`
   strings are presentation-only and never enter the digest; the card id
   never feeds its own digest. No timestamp, absolute checkout spelling, or
   rendered prose field exists on the DTO.
5. Field-for-field projections are documented at each site; the builder owns
   all derivation and renderers consume the card without re-implementing
   any gate or join.
6. The card describes the work order and never marks itself complete:
   observed completion lives in the RepairAttempt/receipt authorities, and
   the claim boundary says exactly that.
7. The eight required shapes (ready, limited, stale, unavailable,
   proposed-target, missing-route, wrong-owner, effect/observer) exist as
   typed builder-test fixtures in the app and domain test modules, matching
   the `fix_instruction` house style; a manifest-only corpus under
   `fixtures/` was considered and rejected as premature because it would
   require an xtask-owned validator before any consumer exists (#4669 may
   revisit when a ratifying consumer lands).

`build_repair_card` projects existing authority only. It performs no IO, no
analysis, no edit, no execution and no network access, and it never
strengthens any readiness, command or attempt claim.

## Required Evidence

- Digest fixtures: a presentation-only command-display change and a
  rewritten `repair_card_id` preserve identity; each load-bearing field
  move (changed behavior, instruction state, readiness flip, selected
  target, each `done_when` axis input, allowed files, attempt section,
  rejected alternatives, snapshot currentness) remints the digest.
- Target fixtures: existing and proposed targets stay distinct variants
  with stable `kind` tags through serde.
- Gate fixtures: the route gate follows the instruction vocabulary and the
  readiness flip together; a not-ready input carrying a command fails
  closed; over-boundary rejected alternatives fail closed.
- Shape fixtures: the eight required shapes each build a card whose
  projected facts match the typed input state.
- Attempt fixture: the manifest's snake_case state string is copied through
  serde without drift or upgrade.

## Non-Goals

- No bounded detail references, no CLI/LSP/MCP projection surface, and no
  field/budget ratification (those land in #4666/#4667/#4669).
- No source or test edit, no provider call, no verification execution, no
  support-tier promotion.
- No removal or weakening of the full canonical packet.
- No new analyzer, readiness validator, instruction state, target-selection
  authority, command authority, or attempt model.

## Acceptance Examples

1. A ready card with a selected existing target and a verify command
   carries the readiness flip, the verbatim instruction, the typed command
   reference, the relation basis, and a minted semantic digest.
2. A stale, limited, or unavailable card carries its instruction state and
   producer-owned blocker but no next action, and a builder input that
   tries to attach one fails closed.
3. A missing-route card has no selected target, no selected basis, and no
   next action.
4. A card with four rejected alternatives fails closed instead of
   truncating.

## Test Mapping

- `crates/ripr/src/repair_card_digest.rs` unit tests cover digest scope and
  stability, the load-bearing field mutation matrix, target-variant
  distinctness, and the five `done_when` axes.
- `crates/ripr/src/domain/repair_card.rs` unit tests cover the route gate
  across all five instruction states.
- `crates/ripr/src/app/repair_card.rs` unit tests cover the ready shape
  field-by-field; stale, limited, unavailable and inspect-only cards
  without routes; fail-closed gates for a closed route, an ineligible
  packet, over-boundary rejections, a basis without a target, a suggested
  assertion without text, and mismatched seam/gap identity; proposed and
  missing targets; wrong-owner basis plus rejections; the effect/observer
  shape; and attempt state copying.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/domain/repair_card.rs` | versioned DTO, sub-types, claim boundary, schema/boundary constants, route gate, tests |
| `crates/ripr/src/repair_card_digest.rs` | scoped digest input, sha256 semantic digest, digest and axis tests |
| `crates/ripr/src/app/repair_card.rs` | `RepairCardInput`, `build_repair_card` projection, fail-closed gates, tests |
| `crates/ripr/src/analysis/test_grip_evidence.rs` | `TestTargetEvidence` projection accessors consumed by the builder |
| `policy/public_api.txt` | public API allowlist entries for the new domain surface |

## Metrics

- `repair_card_schema_version`
- `repair_card_violations`
