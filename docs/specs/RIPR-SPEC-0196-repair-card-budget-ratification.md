# RIPR-SPEC-0196: RepairCard context measurement and default budget ratification

Status: proposed

Owner: product / agent

Created: 2026-10-02

Linked issues:

- #4669 (this slice)
- #3166 (parent: compact RepairCard with progressive evidence references)
- #4663 (RepairCardV1; RIPR-SPEC-0192)
- #4666 (bounded detail references and provisional budget; RIPR-SPEC-0193)
- #4667 (default CLI handoff; RIPR-SPEC-0194)
- #1702 / #1579 (governed real-attempt corpora and counting authority; not
  delivered at ratification time)

Support-tier impact:

- None. The measurement is an advisory, read-only projection over existing
  authorities; it edits nothing, calls no provider, and collects no
  telemetry. [docs/status/SUPPORT_TIERS.md](../status/SUPPORT_TIERS.md)

Policy impact:

- None. No new process, network, or file-policy surface; the report reads
  the committed governed corpus and synthetic profiles and renders to
  stdout/`target/ripr/reports` only.

## Problem

RIPR-SPEC-0193 shipped the RepairCard wire budget (16 detail items, 64 KiB
wire bytes, 4 KiB inline field cap) as provisional-but-versioned numbers, and
RIPR-SPEC-0194 made the card the default CLI agent handoff. A default that
carries provisional numbers is not yet a trustworthy default: #4669 measures
the card's real context behavior and ratifies (or adjusts, with evidence)
the default field set and byte budget. The measurement must be honest about
its denominator: the governed real-attempt corpora (#1702/#1579) are the
only real-opportunity authority, and until they carry attempt cases the
ratification can only cover synthetic fixture scope.

## Behavior

`cargo xtask repair-card-usability-report` (#4669) produces one versioned
`repair_card_usability_report` document:

- **Governed real-opportunity accounting** reuses the shared #1702/#1579
  counting authority (`metrics/rust-repair-trust/corpus.json`) and never
  creates a new denominator. Attempt cases, exclusions, and observations
  are counted as-is; missing, limited, stale, wrong-target,
  archaeology-assisted, and abandoned opportunities stay in the denominator.
  With zero attempt cases the real card measurement reports
  `not_measurable` with an exact reason; a non-zero count flips it to
  `measurable` and forces a receipt refresh.
- **Synthetic fixture measurement** is reported separately and cannot
  ratify real usability by itself. Four deterministic profiles (no witness;
  full witness; witness with a current attempt; witness with a stale
  attempt) are assembled through the same `assemble_repair_card` producer
  and packet renderer the CLI handoff uses. Per profile the report records
  actual normalized UTF-8 byte counts (pretty JSON with exactly one trailing
  newline) for the wire card and the complete canonical packet envelope,
  the packet/card percentage, detail-item count, human-presentation
  line/byte counts, whether the card stays inside the default item and byte
  bounds, whether the wire card omits the packet envelope, whether the
  packet envelope surfaces the seam, the canonical-packet detail state, and
  whether the next action is present.
- **Presentation channels are measured separately from shared semantic
  content**: the CLI JSON and CLI human shapes are measured per profile;
  LSP and MCP card projections are `not_projected` (#4668/#3089/#3090) and
  no LSP/MCP presentation is measured or implied.
- **Field-set decision**: no default field is removed. The wrong-target and
  hidden-help incident rates that would sanction a removal are
  `not_measurable` without governed real attempts, and every default family
  is exercised by the synthetic profiles; the field set stays the
  RIPR-SPEC-0192 default verbatim.
- **Ratified defaults** name the domain constants verbatim (16 items,
  65 536 wire bytes, 4 096 inline bytes) and the #4666 provisional origin.

The committed evidence artifacts under `metrics/repair-card-usability/` pin
the ratification:

- `ratified-expectations.json` pins the synthetic profile set, the
  load-bearing relations (card within both default bounds, wire card omits
  the packet envelope, packet envelope surfaces the seam), and the default
  bounds. The report gate fails closed when a profile violates a relation or
  the profile set, relation set, or bounds drift. The card-vs-packet size
  comparison is reported per profile (`card_bytes_below_packet_bytes`,
  `packet_over_card_percent`) but is not a ratification relation: on these
  single-seam profiles the compact card wire is not smaller than the
  single-seam packet wire, because both are small and the card carries its
  envelope, nine detail references, and digests.
- `decision-receipt.json` is the versioned decision receipt: ratified
  defaults, explicit limitations, combinations not exercised, the synthetic
  evidence pointer, the reused denominator authority, and
  `real_usability_ratification: pending` while the governed corpus carries
  zero attempt cases. The receipt is rejected when it disagrees with the
  live measurement, omits limitations or not-exercised combinations, or
  claims any real-usability ratification state once attempt cases appear —
  no ratification value is accepted until per-opportunity real card
  measurement lands in the same change.

## Required Evidence

- `cargo xtask repair-card-usability-report` rebuilds the measurement in CI,
  validates the committed expectations and decision receipt against the live
  report, and writes `target/ripr/reports/repair-card-usability.{md,json}`
  with the actual normalized byte counts.
- `cargo xtask rust-repair-trust-report` keeps the reused denominator
  authority itself honest.
- `cargo test -p ripr --lib repair_card_usability` covers report
  determinism (byte-identical across runs), per-profile relations, the
  zero-cases `not_measurable` state and its measurable flip, and the
  ratified-defaults/constant agreement.
- `cargo test -p xtask repair_card_usability` covers the gate: committed
  evidence validates against the live measurement, drifted bounds,
  limitation-less receipts, stale pending states, dropped profiles, and
  drifted default bounds are all rejected.
- `cargo xtask check-output-contracts`, `check-support-tiers`,
  `check-capabilities`, `check-evidence-promotion-honesty`, and
  `git diff --check` stay green (the #4669 Proof set).

## Non-Goals

- No universal model benchmark, prompt tuning programme, or automatic
  product-policy change; no model/provider ranking, support-tier promotion,
  or repair-correctness claim is inferred from the sample.
- No autonomous edit, provider call, or hidden telemetry collection.
- No new real-attempt denominator: #1702/#1579 remain the only real-opportunity
  authority, and this slice consumes them read-only.
- No LSP/MCP card projection (that is #4668) and no budget constant change
  without measured evidence and a bumped decision receipt.
- No release or publication action.

## Acceptance Examples

1. With the governed corpus at zero attempt cases, the report states
   `card_measurement_state: not_measurable`, counts exclusions and
   observations in the denominator, and the receipt keeps real usability
   ratification `pending`; synthetic measurements are visibly separate.
2. Each synthetic profile row carries actual normalized byte counts for the
   card and the complete packet, and every load-bearing relation holds;
   removing a relation from the committed expectations fails the gate.
3. A hand-edited receipt that drifts from the domain constants, drops the
   limitations, or stays pending after attempt cases appear fails the
   report gate with an exact reason.
4. The CLI human presentation is counted separately from the JSON wire
   shape, and the LSP/MCP rows read `not_projected` rather than borrowing
   CLI numbers.
5. The ratified defaults equal the domain constants verbatim, so a future
   budget change must touch the constants, the receipt, and the pinned
   expectations together.

## Test Mapping

- `crates/ripr/src/app/repair_card_usability.rs` unit tests: determinism,
  per-profile relations, real-opportunity accounting states, constant
  agreement.
- `xtask/src/reports/repair_card_usability.rs` tests: committed-evidence
  validation and every rejection path.
- `cargo xtask repair-card-usability-report` is itself a CI-executable proof
  of the live measurement and the receipt agreement.

## Implementation Mapping

| Surface | Responsibility |
| --- | --- |
| `crates/ripr/src/app/repair_card_usability.rs` | synthetic profiles, card/packet wire-size measurement, governed real-opportunity accounting, report assembly, unit tests |
| `xtask/src/reports/repair_card_usability.rs` | corpus/evidence loading, expectations + receipt validation, report rendering, gate tests |
| `metrics/repair-card-usability/ratified-expectations.json` | pinned profile set, relations, default bounds |
| `metrics/repair-card-usability/decision-receipt.json` | versioned ratification decision with limitations and not-exercised combinations |
| `xtask/src/command.rs` + `dispatch.rs` | `repair-card-usability-report` registration |

## Metrics

- `repair_card_usability_profiles`
- `repair_card_usability_relations`
- `repair_card_real_attempt_cases`
- `repair_card_budget_receipt_status`
