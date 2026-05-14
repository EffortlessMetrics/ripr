# RIPR-PROP-0006: Rust Usable Gap Projection

Status: proposed

Owner: ripr maintainers

Created: 2026-05-13

Target campaign: Rust usable gap projection

Linked specs:

- `RIPR-SPEC-0045`: Finding-to-gap alignment
- `RIPR-SPEC-0046`: Gap decision ledger
- `RIPR-SPEC-0017`: RIPR Zero reporting
- `RIPR-SPEC-0018`: PR evidence ledger
- `RIPR-SPEC-0020`: First useful action report
- `RIPR-SPEC-0023`: PR review front panel report
- `RIPR-SPEC-0025`: PR inline comment publisher
- `RIPR-SPEC-0036`: Editor preview routing
- `RIPR-SPEC-0037`: Editor preview static-limit projection
- `RIPR-SPEC-0038`: Generated PR CI review workflow
- `RIPR-SPEC-0043`: Presentation text evidence

Linked ADRs:

- None planned for the proposal slice.

Linked work items:

- To be recorded when the campaign manifest opens.

## Problem

RIPR now has enough evidence, policy, PR cockpit, editor, preview-language, and
receipt infrastructure to be useful. The remaining product risk is projection
drift: badges, gates, PR comments, LSP diagnostics, CI summaries, and agent
packets can still read raw findings or report-specific repair routes slightly
differently.

The target user does not need another raw finding. In a PR, they need a single
repairable gap story:

```text
This is a real RIPR gap.
This is the repair route.
This is where it may surface.
This is what proves movement.
```

Without a first-class gap decision layer, downstream surfaces can accidentally
turn `exposed`, `static_unknown`, confidence scores, or preview facts into user
instructions. That creates poor comments, noisy gates, misleading badges, and
agent packets that require too much inference.

This proposal opens the Rust-first lane that separates evidence truth from
projection:

```text
EvidenceClass -> GapDecision -> Projection
```

The lane should make Rust gap projection reliable enough that a PR interruption
is usually worth the reviewer's time.

## Users and surfaces

- Reviewers reading PR comments, generated CI summaries, and front panels.
- Developers repairing one targeted Rust test or output-contract gap.
- Coding agents consuming repair packets and verification commands.
- Maintainers deciding whether `ripr 0`, `ripr+`, and optional gates are safe
  to trust.
- Platform owners separating repo-scoped public badges from PR-local advisory
  evidence.
- Future editor and CI projection work that should consume the same gap
  decision instead of raw evidence.

Primary surfaces:

- Rust CLI and JSON/Markdown report outputs;
- PR evidence ledger and first useful action;
- optional PR inline comments;
- RIPR Zero and badge status;
- optional gate decision input;
- LSP/editor diagnostics and local work packets;
- agent repair packets and receipts.

## Success criteria

- A typed gap decision ledger exists and is the source of projectable Rust gaps.
- Each projectable gap records kind, scope, policy state, evidence IDs, stable
  anchor, repair route, verification command, projection eligibility, and
  receipt path when available.
- Raw static classifications do not directly drive PR comments, gates, badges,
  LSP diagnostics, or agent packets.
- `ripr 0` means zero unresolved, unsuppressed, unwaived, repo-scoped,
  policy-targeted Rust gaps.
- `ripr+` means zero broader unresolved Rust advisory exposure gaps.
- Optional gates only block on new, PR-local, unsuppressed, unwaived,
  non-baseline, non-preview, repairable, policy-targeted Rust gaps.
- PR comments render as repair cards, not raw classifier labels or generic
  confidence text.
- Presentation/output text changes can route to a `MissingOutputContract` gap
  with an `AddOutputGolden` or equivalent output-proof repair route.
- LSP and agent packets can request the same gap record that PR comments and CI
  summaries reference.
- Receipts prove movement for the selected gap decision, not merely report
  existence.

## Proposed shape

Introduce a first-class gap decision object above evidence records and below
projection surfaces:

```rust
GapRecord {
    kind,
    scope,
    policy_state,
    repair_route,
    evidence_ids,
    anchor,
    projection_eligibility,
    verification_commands,
    receipt_path,
}
```

The exact Rust and JSON shape belongs in the planned gap ledger spec. The
proposal-level rule is that all public projection surfaces should consume a
gap decision when deciding whether to interrupt a reviewer, block an optional
gate, change a badge count, show an LSP diagnostic, or hand work to an agent.

The decision layer is Rust-first. TypeScript, Python, and other preview
languages remain opt-in and advisory; they may be visible in preview reports,
but they must not become `ripr 0`, `ripr+`, or default gate truth in this lane.

Presentation/output text evidence from `RIPR-SPEC-0043` is a motivating
consumer. It should become a repairable output-contract gap only when the
evidence supports that route. Unknown presentation text remains a static
limitation, not a mutation-testing instruction.

## Alternatives considered

| Alternative | Why we are not picking it |
| --- | --- |
| Let each projection surface filter raw findings. | This preserves drift and makes every surface rediscover policy, baseline, waiver, preview, repairability, and receipt rules. |
| Treat "no findings" as the badge target. | Raw count zero is not an operational promise and can over-trust unsupported or unrepairable evidence. |
| Make gates consume scores or confidence thresholds directly. | A useful gate should block on a new repairable policy-targeted gap, not on score theater. |
| Suppress noisy comments only at the publisher. | Suppression hides symptoms but does not give PR comments, LSP, gates, and agent packets the same repair language. |
| Put presentation text directly into PR comments. | Output text needs an evidence-backed output-contract repair route first; raw string changes are not always user test debt. |
| Promote preview-language evidence into the same ledger now. | Preview evidence is useful, but Rust usable projection is the trust bottleneck and default adoption path. |

## Behavior specs to create or update

- Use `RIPR-SPEC-0045` as the evidence-alignment input contract.
- Add `RIPR-SPEC-0046`: Gap decision ledger.
- Update `RIPR-SPEC-0017` so RIPR Zero and badge targets consume policy-backed
  gap decisions instead of raw report counts.
- Update `RIPR-SPEC-0018` and `RIPR-SPEC-0020` so the PR evidence ledger and
  first useful action carry gap IDs, repair routes, verification commands, and
  receipts from the same decision layer.
- Update `RIPR-SPEC-0025` so PR inline comments require a repairable
  `GapRecord` and render as repair cards.
- Update `RIPR-SPEC-0023` and `RIPR-SPEC-0038` so front panel and generated CI
  summary projections preserve the gap decision boundary.
- Update `RIPR-SPEC-0036` and `RIPR-SPEC-0037` when LSP/editor packets consume
  gap decisions instead of raw evidence.
- Link `RIPR-SPEC-0043` as the presentation/output text evidence source for
  `MissingOutputContract` style gap decisions.

## Architecture decisions needed

No ADR is required for this proposal slice. Add an ADR only if the
implementation changes a durable architecture boundary, such as making the gap
ledger a persisted repo database, splitting public crates, or redefining
policy authority. The default should be additive reports and internal typed
models inside the existing `ripr` package.

## Implementation campaign shape

Each slice follows the [scoped PR contract](../SCOPED_PR_CONTRACT.md).

1. `docs/proposal-rust-usable-gap-projection`
2. `docs/spec-gap-decision-ledger`
3. `fixtures/gap-decision-ledger-corpus`
4. `output/gap-record-model-and-ledger`
5. `report/first-useful-action-gap-record`
6. `report/pr-evidence-ledger-gap-record`
7. `policy/ripr-zero-and-badges-gap-targets`
8. `policy/gate-repairable-gap-predicate`
9. `comments/repair-card-projection`
10. `editor/gap-work-packet-projection`
11. `analysis/presentation-output-contract-gap-route`
12. `dogfood/gap-projection-repair-receipts`
13. `campaign/rust-usable-gap-projection-closeout`

Keep implementation slices Rust-first and projection-safe. Do not mix badge,
gate, PR comment, LSP, and presentation-text behavior in one production PR.

## Evidence plan

- Gap ledger spec defines the typed decision vocabulary, required fields,
  projection eligibility, and safe gate predicate.
- Fixture corpus covers repairable Rust boundary gaps, baseline debt, waived
  gaps, suppressed gaps, blocked gates, static limitations, presentation/output
  text, preview-language non-eligibility, missing receipts, improved receipts,
  and unchanged-after-attempt receipts.
- JSON and Markdown goldens pin the gap ledger output before projection
  surfaces consume it.
- PR comment fixtures prove repair-card copy, dedupe fingerprints, stable
  anchors, verification commands, and no raw `static_unknown` instruction.
- RIPR Zero and badge tests prove repo-scoped targets do not imply PR-local
  adequacy.
- Gate tests prove only new repairable policy-targeted Rust gaps can block.
- LSP and agent packet fixtures prove local work packets reference the same gap
  IDs and repair routes.
- Receipts prove movement for the selected `GapRecord`.
- Capability, traceability, support-tier, and closeout docs record what became
  usable, what remains advisory, and what still belongs to runtime mutation
  calibration.

## Risks

- The gap ledger could become another projection surface instead of the common
  decision source. Mitigation: downstream specs must state that projections
  consume it and do not invent eligibility.
- Badges could overclaim PR-local adequacy. Mitigation: badge semantics remain
  repo-scoped and policy-backed.
- Gates could block on unrepairable or unknown evidence. Mitigation: the safe
  gate predicate requires a repair route and excludes preview, baseline,
  waived, suppressed, and static-unknown-only records.
- PR comments could become verbose. Mitigation: comments require a stable
  anchor, dedupe fingerprint, repair route, and verification command.
- Presentation text could become generic snapshot-test churn. Mitigation:
  `RIPR-SPEC-0043` remains the evidence source; a missing output contract gap
  requires supported visibility and observer evidence.
- Agent packets could drift from human projections. Mitigation: packets carry
  the same gap ID, repair route, and receipt path as PR/CI surfaces.

## Non-goals

- No broad analyzer accuracy campaign in this proposal slice.
- No runtime mutation execution.
- No coverage adequacy or general correctness claims.
- No generated tests.
- No automatic source edits.
- No provider or model calls.
- No default CI blocking change.
- No branch protection change.
- No preview-language promotion into gates, RIPR Zero, or public badge truth.
- No schema fork; output changes must be additive and spec-backed.
- No LSP feature expansion before the gap work-packet contract exists.
- No replacement of existing evidence records; the gap ledger composes them.

## Exit criteria

This proposal can move to `accepted` when:

- the gap decision ledger spec lands;
- a fixture-backed gap ledger report or equivalent public artifact exists;
- first useful action and PR evidence ledger consume gap IDs and repair routes
  from that decision layer;
- PR comments render repair cards from gap decisions and stop rendering raw
  classifier labels as instructions;
- RIPR Zero and badge targets are policy-backed gap targets, not raw finding
  counts;
- optional gate evaluation consumes the safe repairable-gap predicate;
- editor or agent work packets can reference the same gap record;
- presentation/output text can route to an output-contract repair when
  evidence supports it;
- receipts prove before/after movement for selected gap records;
- support tiers, capability matrix, traceability, and closeout artifacts state
  the remaining advisory and runtime-mutation boundaries.
